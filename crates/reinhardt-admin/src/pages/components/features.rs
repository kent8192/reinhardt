//! Feature-specific components
//!
//! Provides feature-specific UI components:
//! - `Dashboard` - Dashboard view
//! - `ListView` - List view with filters and pagination
//! - `DetailView` - Detail view for a single record
//! - `ModelForm` - Form for creating/editing records
//! - `Filters` - Filter panel
//! - `DataTable` - Data table component

use crate::types::{
	AdminAction, FieldInfo, FieldType, Fieldset, FilterInfo, FilterType, HistoryResponse,
	InlineEditResponse, InlineFormInfo, InlineRowInfo, InlineStyle, ModelInfo, MutationResponse,
	PrepopulatedField, RelationOption, RelationWidget,
};
#[cfg(client)]
use crate::types::{AdminActionRequest, RelationLookupRequest};
#[cfg(any(client, test))]
use crate::types::{
	DateHierarchyInfo, DateHierarchyLevel, DateHierarchyListQueryParams, DateHierarchySelection,
};
#[cfg(client)]
use reinhardt_pages::ResourceState;
use reinhardt_pages::component::{IntoPage, Page, PageElement};
use reinhardt_pages::event::{ChangeEvent, EventPayload, typed_event_handler};
use reinhardt_pages::page;
use reinhardt_pages::{Action, EventType, IntoEventHandler, Signal};
#[cfg(any(client, test))]
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;

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
	/// Whether an editable value accepts the database NULL state.
	pub nullable: bool,
	/// Optional HTML numeric step for editable controls.
	pub step: Option<String>,
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
///         Column {
///             field: "id".to_string(),
///             label: "ID".to_string(),
///             sortable: true,
///             editable: false,
///             linked: true,
///             required: true,
///             nullable: false,
///             step: None,
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
	list_view_content(
		data,
		&data.pk_field,
		&[],
		current_page_signal,
		filters_signal,
		Page::empty(),
		None,
		None,
	)
}

/// Renders the router-owned list view with optional date hierarchy navigation.
#[cfg(any(client, test))]
pub(crate) fn list_view_with_date_hierarchy(
	data: &ListViewData,
	current_page_signal: Signal<u64>,
	filters_signal: Signal<HashMap<String, String>>,
	date_hierarchy: Option<&DateHierarchyInfo>,
	query_params: Signal<DateHierarchyListQueryParams>,
	query_generation: Rc<Cell<u64>>,
) -> Page {
	let date_hierarchy_page =
		date_hierarchy_navigation(date_hierarchy, query_params, query_generation);
	list_view_content(
		data,
		&data.pk_field,
		&[],
		current_page_signal,
		filters_signal,
		date_hierarchy_page,
		None,
		None,
	)
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
		Page::empty(),
		Some(action_state),
		None,
	)
}

#[cfg(client)]
pub(crate) fn list_view_with_actions_and_edit(
	data: &ListViewData,
	pk_field: &str,
	actions: &[AdminAction],
	current_page_signal: Signal<u64>,
	filters_signal: Signal<HashMap<String, String>>,
	date_hierarchy: Option<&DateHierarchyInfo>,
	query_params: Signal<DateHierarchyListQueryParams>,
	query_generation: Rc<Cell<u64>>,
	action_state: ListActionState,
	save_action: Action<InlineEditResponse, String>,
) -> Page {
	let date_hierarchy_page =
		date_hierarchy_navigation(date_hierarchy, query_params, query_generation);
	list_view_content(
		data,
		pk_field,
		actions,
		current_page_signal,
		filters_signal,
		date_hierarchy_page,
		Some(action_state),
		Some(save_action),
	)
}

fn record_primary_key(
	record: &HashMap<String, serde_json::Value>,
	pk_field: &str,
) -> Option<String> {
	scalar_object_id(record.get(pk_field))
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
	date_hierarchy_page: Page,
	action_state: Option<ListActionState>,
	save_action: Option<reinhardt_pages::Action<InlineEditResponse, String>>,
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
	 date_hierarchy_page: Page,
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
			{ date_hierarchy_page }
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
		date_hierarchy_page,
		action_controls,
		summary,
		table_page,
		pagination_page,
	)
}

#[cfg(any(client, test))]
fn apply_date_hierarchy_choice(
	query_params: Signal<DateHierarchyListQueryParams>,
	query_generation: Rc<Cell<u64>>,
	mut selection: DateHierarchySelection,
	next_level: DateHierarchyLevel,
	choice: i32,
) {
	match next_level {
		DateHierarchyLevel::Year => {
			selection.year = Some(choice);
			selection.month = None;
			selection.day = None;
		}
		DateHierarchyLevel::Month => {
			let Ok(month) = u32::try_from(choice) else {
				return;
			};
			selection.month = Some(month);
			selection.day = None;
		}
		DateHierarchyLevel::Day => {
			let Ok(day) = u32::try_from(choice) else {
				return;
			};
			selection.day = Some(day);
		}
	}

	apply_date_hierarchy_selection(query_params, query_generation, Some(selection));
}

#[cfg(any(client, test))]
fn apply_date_hierarchy_selection(
	query_params: Signal<DateHierarchyListQueryParams>,
	query_generation: Rc<Cell<u64>>,
	selection: Option<DateHierarchySelection>,
) {
	let mut params = query_params.get_untracked();
	if params.page == Some(1) && params.date_hierarchy == selection {
		return;
	}
	params.page = Some(1);
	params.date_hierarchy = selection;
	query_generation.set(query_generation.get().wrapping_add(1));
	query_params.set(params);
}

#[cfg(any(client, test))]
fn date_hierarchy_breadcrumbs(
	selection: &DateHierarchySelection,
	query_params: Signal<DateHierarchyListQueryParams>,
	query_generation: Rc<Cell<u64>>,
) -> Vec<Page> {
	let mut breadcrumbs = vec![page!(|query_params: Signal<DateHierarchyListQueryParams>,
	 query_generation: Rc<Cell<u64>>| {
		button {
			type: "button",
			class: "admin-btn admin-btn-outline admin-btn-sm",
			aria_label: "Clear date hierarchy",
			@click: move |_| {
				crate::pages::components::features::apply_date_hierarchy_selection(
					query_params,
					query_generation.clone(),
					None,
				);
			},
			"All"
		}
	})(query_params, query_generation.clone())];

	if let Some(year) = selection.year {
		breadcrumbs.push(page!(|query_params: Signal<DateHierarchyListQueryParams>,
		 query_generation: Rc<Cell<u64>>,
		 year: i32| {
			button {
				type: "button",
				class: "admin-btn admin-btn-outline admin-btn-sm",
				aria_label: "Back to year",
				@click: move |_| {
					crate::pages::components::features::apply_date_hierarchy_selection(
						query_params,
						query_generation.clone(),
						Some(DateHierarchySelection {
							year: Some(year),
							month: None,
							day: None,
						}),
					);
				},
				{ year.to_string() }
			}
		})(query_params, query_generation.clone(), year));
	}

	if let (Some(year), Some(month)) = (selection.year, selection.month) {
		breadcrumbs.push(page!(|query_params: Signal<DateHierarchyListQueryParams>,
		 query_generation: Rc<Cell<u64>>,
		 year: i32,
		 month: u32| {
			button {
				type: "button",
				class: "admin-btn admin-btn-outline admin-btn-sm",
				aria_label: "Back to month",
				@click: move |_| {
					crate::pages::components::features::apply_date_hierarchy_selection(
						query_params,
						query_generation.clone(),
						Some(DateHierarchySelection {
							year: Some(year),
							month: Some(month),
							day: None,
						}),
					);
				},
				{ format!("Month {month}") }
			}
		})(query_params, query_generation.clone(), year, month));
	}

	breadcrumbs
}

#[cfg(any(client, test))]
fn date_hierarchy_navigation(
	date_hierarchy: Option<&DateHierarchyInfo>,
	query_params: Signal<DateHierarchyListQueryParams>,
	query_generation: Rc<Cell<u64>>,
) -> Page {
	let Some(date_hierarchy) = date_hierarchy else {
		return Page::empty();
	};

	let field = date_hierarchy.field.clone();
	let breadcrumbs = date_hierarchy_breadcrumbs(
		&date_hierarchy.selection,
		query_params,
		query_generation.clone(),
	);
	let choices = date_hierarchy
		.next_level
		.map_or_else(Vec::new, |next_level| {
			let level_label = match next_level {
				DateHierarchyLevel::Year => "year",
				DateHierarchyLevel::Month => "month",
				DateHierarchyLevel::Day => "day",
			};
			date_hierarchy
				.choices
				.iter()
				.map(|choice| {
					let choice = *choice;
					let label = choice.to_string();
					let aria_label = format!("Select {level_label} {choice}");
					let selection = date_hierarchy.selection.clone();
					page!(|label: String,
					 aria_label: String,
					 _query_params: Signal<DateHierarchyListQueryParams>,
					 _query_generation: Rc<Cell<u64>>,
					 _selection: DateHierarchySelection,
					 _next_level: DateHierarchyLevel,
					 _choice: i32| {
						button {
							type: "button",
							class: "admin-btn admin-btn-outline admin-btn-sm",
							aria_label: aria_label,
							@click: move |_| {
								crate::pages::components::features::apply_date_hierarchy_choice(
									_query_params,
									_query_generation.clone(),
									_selection.clone(),
									_next_level,
									_choice,
								);
							},
							{ label }
						}
					})(
						label,
						aria_label,
						query_params,
						query_generation.clone(),
						selection,
						next_level,
						choice,
					)
				})
				.collect()
		});

	page!(|field: String, breadcrumbs: Vec<Page>, choices: Vec<Page>| {
		nav {
			class: "admin-card p-4 mb-4",
			aria_label: "Date hierarchy",
			h2 {
				class: "text-xs font-semibold uppercase tracking-wider text-slate-500 mb-3",
				"Date hierarchy"
			}
			p {
				class: "text-sm text-slate-600 mb-3",
				{ field }
			}
			div {
				class: "flex flex-wrap gap-2 mb-3",
				{ breadcrumbs }
			}
			div {
				class: "flex flex-wrap gap-2",
				{ choices }
			}
		}
	})(field, breadcrumbs, choices)
}

/// Generates a data table
fn data_table(
	columns: &[Column],
	records: &[std::collections::HashMap<String, serde_json::Value>],
	model_name: &str,
	pk_field: &str,
	selection: Option<ListSelectionState>,
	save_action: Option<reinhardt_pages::Action<InlineEditResponse, String>>,
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
		.map(|record| {
			table_row(
				columns,
				record,
				model_name,
				pk_field,
				selection,
				save_action,
			)
		})
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
	selection: Option<ListSelectionState>,
	save_action: Option<reinhardt_pages::Action<InlineEditResponse, String>>,
) -> Page {
	let record_id = scalar_object_id(record.get(pk_field));
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
				return editable_table_cell(model_name, col, &value, object_id, save_action);
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
	let selection_cells = selection_cell.into_iter().collect::<Vec<_>>();

	page!(|selection_cells: Vec<Page>, data_cells: Vec<Page>, actions_cell: Page, row_key: String| {
		tr {
			{ selection_cells }data_row_pk: row_key,
			{ data_cells }
			{ actions_cell }
		}
	})(selection_cells, data_cells, actions_cell, row_key)
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
	NullableBoolean,
	Array,
	Json,
	Time,
	DateTime,
}

impl InlineValueKind {
	fn as_str(self) -> &'static str {
		match self {
			Self::String => "string",
			Self::Number => "number",
			Self::Boolean => "boolean",
			Self::NullableBoolean => "nullable-boolean",
			Self::Array => "array",
			Self::Json => "json",
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
			"nullable-boolean" => Some(Self::NullableBoolean),
			"array" => Some(Self::Array),
			"json" => Some(Self::Json),
			"time" => Some(Self::Time),
			"datetime" => Some(Self::DateTime),
			_ => None,
		}
	}
}

fn nullable_boolean_choices() -> Vec<(String, String)> {
	vec![
		(String::new(), "Unset".to_string()),
		("true".to_string(), "True".to_string()),
		("false".to_string(), "False".to_string()),
	]
}

fn is_nullable_boolean_choices(choices: &[(String, String)]) -> bool {
	choices.len() == 3
		&& choices[0].0.is_empty()
		&& choices[1].0 == "true"
		&& choices[2].0 == "false"
}

fn inline_value_kind(
	spec: &crate::types::FormFieldSpec,
	value: &serde_json::Value,
) -> InlineValueKind {
	match spec {
		crate::types::FormFieldSpec::Input { html_type } if html_type == "checkbox" => {
			InlineValueKind::Boolean
		}
		crate::types::FormFieldSpec::Select { choices } if is_nullable_boolean_choices(choices) => {
			InlineValueKind::NullableBoolean
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
		crate::types::FormFieldSpec::Json => InlineValueKind::Json,
		crate::types::FormFieldSpec::MultiSelect { .. } => InlineValueKind::Array,
		_ if value.is_array() || value.is_object() => InlineValueKind::Json,
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
			InlineValueKind::Time => {
				normalized_time_input_value(value).map(serde_json::Value::String)
			}
			InlineValueKind::DateTime => {
				normalized_datetime_input_value(value).map(serde_json::Value::String)
			}
			InlineValueKind::Array => Some(serde_json::Value::Array(
				parse_multi_value(value)
					.into_iter()
					.map(|value| serde_json::Value::String(value.to_string()))
					.collect(),
			)),
			_ => None,
		};
		if let Some(normalized) = normalized {
			return normalized;
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
		InlineValueKind::NullableBoolean | InlineValueKind::Json => serde_json::Value::Null,
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
		value.format("%H:%M:%S%.f").to_string()
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
	model_name: &str,
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
	let spec = if column.nullable && value_kind == InlineValueKind::Boolean {
		crate::types::FormFieldSpec::Select {
			choices: nullable_boolean_choices(),
		}
	} else {
		spec
	};
	let value_kind = inline_value_kind(&spec, value);
	let control_value = normalized_inline_original(value, value_kind);
	let display_value = if value_kind == InlineValueKind::Json {
		if control_value.is_null() {
			String::new()
		} else {
			control_value.to_string()
		}
	} else {
		json_value_to_display_string(&control_value)
	};
	let field = FormField {
		name: column.field.clone(),
		label: column.label.clone(),
		spec,
		required: column.required,
		nullable: column.nullable,
		value: display_value,
	};
	let input = form_element_with_description_for_model_and_step(
		model_name,
		&field,
		&input_id,
		&label,
		&error_id,
		column.step.as_deref(),
	);
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
const INLINE_JSON_VALUE_KEY: &str = "__reinhardt_inline_json_value__";

#[cfg(any(client, test))]
fn tagged_inline_json_value(value: serde_json::Value) -> serde_json::Value {
	serde_json::json!({ INLINE_JSON_VALUE_KEY: value })
}

#[cfg(any(client, test))]
fn take_tagged_inline_json_value(value: serde_json::Value) -> (serde_json::Value, bool) {
	let serde_json::Value::Object(mut object) = value else {
		return (value, false);
	};
	if object.len() != 1 || !object.contains_key(INLINE_JSON_VALUE_KEY) {
		return (serde_json::Value::Object(object), false);
	}
	(
		object
			.remove(INLINE_JSON_VALUE_KEY)
			.expect("tagged inline JSON contains its value"),
		true,
	)
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
		let (current, is_json) = take_tagged_inline_json_value(snapshot.current);
		let is_dirty = original != current || (is_json && current.is_null());
		if object_id.is_empty() || field.is_empty() || !is_dirty {
			continue;
		}

		let position = *positions.entry(object_id.clone()).or_insert_with(|| {
			updates.push(crate::types::InlineEditMutation {
				object_id,
				changes: HashMap::new(),
				json_fields: Vec::new(),
			});
			updates.len() - 1
		});
		if is_json {
			updates[position].json_fields.push(field.clone());
		}
		updates[position].changes.insert(field, current);
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
	let Some(snapshots) = collect_inline_control_snapshots(event.raw()) else {
		save_action.reset();
		return;
	};
	let Some(request) = inline_edit_request(
		reinhardt_pages::csrf::get_csrf_token().unwrap_or_default(),
		snapshots,
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
fn collect_inline_control_snapshots(event: &web_sys::Event) -> Option<Vec<InlineControlSnapshot>> {
	use wasm_bindgen::JsCast;

	let target = event.target().or_else(|| event.current_target());
	let Some(form) = target.and_then(|target| target.dyn_into::<web_sys::HtmlFormElement>().ok())
	else {
		return Some(Vec::new());
	};

	let elements = form.elements();
	let mut snapshots = Vec::new();
	for index in 0..elements.length() {
		let Some(element) = elements.item(index) else {
			continue;
		};
		let Some(cell) = element.parent_element() else {
			continue;
		};
		if cell.get_attribute("data-inline-editable").as_deref() != Some("true") {
			continue;
		}
		let Some(value_kind) = cell
			.get_attribute("data-inline-value-kind")
			.and_then(|value| InlineValueKind::parse(&value))
		else {
			continue;
		};
		let current = inline_form_control_value(&element, value_kind)?;
		snapshots.push(InlineControlSnapshot {
			object_id: cell.get_attribute("data-object-id"),
			field: cell.get_attribute("data-field"),
			original: cell
				.get_attribute("data-original-json")
				.and_then(|value| serde_json::from_str(&value).ok()),
			current,
		});
	}
	Some(snapshots)
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
		if kind == InlineValueKind::Json {
			let value = textarea.value();
			if value.trim().is_empty() {
				textarea.set_custom_validity("");
				return Some(serde_json::Value::Null);
			}
			return match serde_json::from_str(&value) {
				Ok(value) => {
					textarea.set_custom_validity("");
					Some(tagged_inline_json_value(value))
				}
				Err(_) => {
					textarea.set_custom_validity("Enter valid JSON");
					let _ = textarea.report_validity();
					None
				}
			};
		}
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
	match kind {
		InlineValueKind::Number => {
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
		InlineValueKind::NullableBoolean => match value {
			"" => serde_json::Value::Null,
			"true" => serde_json::Value::Bool(true),
			"false" => serde_json::Value::Bool(false),
			_ => serde_json::Value::String(value.to_string()),
		},
		InlineValueKind::Json => {
			if value.trim().is_empty() {
				serde_json::Value::Null
			} else {
				serde_json::from_str(value).unwrap_or(serde_json::Value::Null)
			}
		}
		_ => serde_json::Value::String(value.to_string()),
	}
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
	/// Whether this field may be explicitly cleared
	pub nullable: bool,
	/// Current field value (for edit forms)
	pub value: String,
}

#[derive(Debug, Clone, Default)]
struct FieldPresentation {
	help_text: Option<String>,
	placeholder: Option<String>,
}

fn field_presentations(fields: &[FieldInfo]) -> HashMap<String, FieldPresentation> {
	fields
		.iter()
		.map(|field| {
			(
				field.name.clone(),
				FieldPresentation {
					help_text: field.help_text.clone(),
					placeholder: field.placeholder.clone(),
				},
			)
		})
		.collect()
}

/// Applies unlocked prepopulation rules to the current form values.
fn apply_prepopulated_values(
	values: &mut HashMap<String, String>,
	rules: &[PrepopulatedField],
	dirty_targets: &HashSet<String>,
) {
	for rule in rules {
		if dirty_targets.contains(&rule.target) {
			continue;
		}
		let source_values = rule
			.sources
			.iter()
			.map(|source| values.get(source).map(String::as_str).unwrap_or_default())
			.collect::<Vec<_>>()
			.join(" ");
		values.insert(
			rule.target.clone(),
			reinhardt_utils::utils_core::encoding::slugify(&source_values),
		);
	}
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
	let history_url = admin_record_url("history", model_name, record_id);
	let list_url = admin_model_url("list", model_name);

	let title = format!("{} Detail", model_name);
	let table_page = detail_table(record);
	let edit_link = Link::new(edit_url, "Edit")
		.class("admin-btn admin-btn-primary mr-2")
		.render();
	let history_link = Link::new(history_url, "History")
		.class("admin-btn admin-btn-secondary")
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
	 history_link: Page,
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
				{ history_link }
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
		history_link,
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

/// Paginated change history for one admin object.
pub fn history_view(response: &HistoryResponse, current_page: Signal<u64>) -> Page {
	history_view_with_route_model_name(response, current_page, &response.model_name)
}

pub(crate) fn history_view_with_route_model_name(
	response: &HistoryResponse,
	current_page: Signal<u64>,
	route_model_name: &str,
) -> Page {
	use reinhardt_pages::component::Component;
	use reinhardt_pages::router::Link;

	let title = format!("{} History", response.model_name);
	let summary = format!(
		"{} change{} for {} ({})",
		response.count,
		if response.count == 1 { "" } else { "s" },
		response.model_name,
		response.object_id
	);
	let rows: Vec<Page> = response
		.results
		.iter()
		.map(|entry| {
			let timestamp = entry.timestamp.clone();
			let actor = entry.actor.clone();
			let action_name = entry.action_name.clone();
			let object_repr = entry.object_repr.clone();
			let changed_fields = if entry.changed_fields.is_empty() {
				"—".to_string()
			} else {
				entry.changed_fields.join(", ")
			};
			let affected_count = entry.affected_count.to_string();
			let status = if entry.success { "Succeeded" } else { "Failed" }.to_string();
			page!(|timestamp: String,
			 actor: String,
			 action_name: String,
			 object_repr: String,
			 changed_fields: String,
			 affected_count: String,
			 status: String| {
				tr {
					td { { timestamp } }
					td { { actor } }
					td { { action_name } }
					td { { object_repr } }
					td { { changed_fields } }
					td { { affected_count } }
					td { { status } }
				}
			})(
				timestamp,
				actor,
				action_name,
				object_repr,
				changed_fields,
				affected_count,
				status,
			)
		})
		.collect();
	let history_table = if rows.is_empty() {
		page!(|| {
			div {
				class: "admin-alert admin-alert-info",
				"No history entries found."
			}
		})()
	} else {
		page!(|rows: Vec<Page>| {
			div {
				class: "overflow-x-auto rounded-lg border border-slate-200",
				table {
					class: "admin-table",
					thead {
						tr {
							th { "Timestamp" }
							th { "Actor" }
							th { "Action" }
							th { "Object" }
							th { "Changed fields" }
							th { "Affected" }
							th { "Status" }
						}
					}
					tbody { { rows } }
				}
			}
		})(rows)
	};
	let pagination =
		crate::pages::components::common::pagination(current_page, response.total_pages);
	let back_link = Link::new(admin_model_url("list", route_model_name), "Back to List")
		.class("admin-btn admin-btn-secondary")
		.render();

	page!(|title: String, summary: String, history_table: Page, pagination: Page, back_link: Page| {
		div {
			class: "history-view animate__animated animate__fadeIn",
			h1 {
				class: "font-display text-2xl font-bold text-slate-900 mb-2",
				{ title }
			}
			p {
				class: "text-sm text-slate-500 mb-6",
				{ summary }
			}
			{ history_table }
			{ pagination }
			div {
				class: "mt-6",
				{ back_link }
			}
		}
	})(title, summary, history_table, pagination, back_link)
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
///         nullable: false,
///         value: "".to_string(),
///     },
/// ];
/// model_form("User", &fields, None)
/// ```
pub fn model_form(model_name: &str, fields: &[FormField], record_id: Option<&str>) -> Page {
	model_form_with_configuration(model_name, fields, &[], &[], record_id, &[])
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
	model_form_with_configuration(model_name, fields, fieldsets, &[], record_id, &[])
}

/// Model form component with optional parent fieldsets and related child rows.
pub fn model_form_with_inlines(
	model_name: &str,
	fields: &[FormField],
	fieldsets: &[Fieldset],
	inlines: &[InlineFormInfo],
	record_id: Option<&str>,
) -> Page {
	model_form_with_configuration(model_name, fields, fieldsets, inlines, record_id, &[])
}

/// Model form component with server-validated field prepopulation rules.
pub(crate) fn model_form_with_configuration(
	model_name: &str,
	fields: &[FormField],
	fieldsets: &[Fieldset],
	inlines: &[InlineFormInfo],
	record_id: Option<&str>,
	prepopulated_fields: &[PrepopulatedField],
) -> Page {
	model_form_with_field_info(
		model_name,
		fields,
		fieldsets,
		inlines,
		record_id,
		prepopulated_fields,
		&[],
	)
}

/// Render a model form using server-resolved field presentation metadata.
///
/// This keeps [`FormField`] source-compatible for callers that construct the
/// original five-field value while allowing the admin fields endpoint to pass
/// help text and placeholders separately.
pub fn model_form_with_field_info(
	model_name: &str,
	fields: &[FormField],
	fieldsets: &[Fieldset],
	inlines: &[InlineFormInfo],
	record_id: Option<&str>,
	prepopulated_fields: &[PrepopulatedField],
	field_infos: &[FieldInfo],
) -> Page {
	let mut values = fields
		.iter()
		.map(|field| (field.name.clone(), field.value.clone()))
		.collect::<HashMap<_, _>>();
	let dirty_targets = prepopulated_fields
		.iter()
		.filter(|rule| {
			values
				.get(&rule.target)
				.is_some_and(|value| !value.is_empty())
		})
		.map(|rule| rule.target.clone())
		.collect::<HashSet<_>>();
	apply_prepopulated_values(&mut values, prepopulated_fields, &dirty_targets);
	let fields = fields
		.iter()
		.map(|field| {
			let mut field = field.clone();
			if let Some(value) = values.get(&field.name) {
				field.value.clone_from(value);
			}
			field
		})
		.collect::<Vec<_>>();

	let presentations = field_presentations(field_infos);
	let parent_groups =
		parent_form_groups_with_presentations(model_name, &fields, fieldsets, &presentations);
	if inlines.is_empty() {
		return model_form_page(
			model_name,
			record_id,
			parent_groups,
			prepopulated_fields.to_vec(),
			dirty_targets,
		);
	}

	let mut groups = Vec::with_capacity(inlines.len() + 1);
	groups.push(parent_groups);
	groups.extend(inlines.iter().map(inline_form_section));

	model_form_page(
		model_name,
		record_id,
		Page::Fragment(groups),
		prepopulated_fields.to_vec(),
		dirty_targets,
	)
}

fn parent_form_groups_with_presentations(
	model_name: &str,
	fields: &[FormField],
	fieldsets: &[Fieldset],
	presentations: &HashMap<String, FieldPresentation>,
) -> Page {
	if fieldsets.is_empty() {
		return flat_parent_form_groups(model_name, fields, presentations);
	}

	fieldset_parent_form_groups(model_name, fields, fieldsets, presentations)
}

fn flat_parent_form_groups(
	model_name: &str,
	fields: &[FormField],
	presentations: &HashMap<String, FieldPresentation>,
) -> Page {
	let form_fields: Vec<Page> = fields
		.iter()
		.map(|field| {
			form_group_with_presentation(model_name, field, presentations.get(&field.name))
		})
		.collect();
	page!(|form_fields: Vec<Page>| {
		div {
			class: "admin-card p-6",
			{ form_fields }
		}
	})(form_fields)
}

fn fieldset_parent_form_groups(
	model_name: &str,
	fields: &[FormField],
	fieldsets: &[Fieldset],
	presentations: &HashMap<String, FieldPresentation>,
) -> Page {
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
			let form_fields: Vec<Page> = form_fields
				.into_iter()
				.map(|field| {
					form_group_with_presentation(model_name, field, presentations.get(&field.name))
				})
				.collect();

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
			if field.readonly || (row.id.is_some() && !inline.can_change) {
				return inline_readonly_field(layout, input_id, label, value);
			}
			let form_field = FormField {
				name: inline_field_name(&inline.key, index, &field.name),
				label: label.clone(),
				spec: crate::types::FormFieldSpec::from(&field.field_type),
				required: row.id.is_some() && field.required,
				nullable: field.nullable,
				value,
			};
			let presentation = FieldPresentation {
				help_text: field.help_text.clone(),
				placeholder: field.placeholder.clone(),
			};
			let input = form_element_with_presentation_for_model(
				&inline.model_name,
				&form_field,
				&input_id,
				&label,
				"",
				Some(&presentation),
			);
			let input = if row.id.is_none() && !field.readonly {
				inline_presence_tracking_input(&inline.key, index, input)
			} else {
				input
			};

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

fn inline_presence_tracking_input(key: &str, index: usize, input: Page) -> Page {
	let presence_id = inline_field_id(key, index, "__present");
	page!(|input: Page, input_presence_id: String, change_presence_id: String| {
		span {
			@input: move |_| {
				#[cfg(client)]
				crate::pages::components::features::mark_inline_row_present(&input_presence_id);
			},
			@change: move |_| {
				#[cfg(client)]
				crate::pages::components::features::mark_inline_row_present(&change_presence_id);
			},
			{ input }
		}
	})(input, presence_id.clone(), presence_id)
}

#[cfg(client)]
fn mark_inline_row_present(presence_id: &str) {
	use wasm_bindgen::JsCast;

	let Some(document) = web_sys::window().and_then(|window| window.document()) else {
		return;
	};
	let Some(element) = document.get_element_by_id(presence_id) else {
		return;
	};
	let Ok(input) = element.dyn_into::<web_sys::HtmlInputElement>() else {
		return;
	};
	input.set_value("true");
}

#[cfg(client)]
fn set_inline_required_fields(field_ids: &[String], deleted: bool) {
	let Some(document) = web_sys::window().and_then(|window| window.document()) else {
		return;
	};
	for field_id in field_ids {
		let Some(element) = document.get_element_by_id(field_id) else {
			continue;
		};
		if deleted {
			let _ = element.remove_attribute("required");
		} else {
			let _ = element.set_attribute("required", "");
		}
	}
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
	let presence_name = inline_field_name(&inline.key, index, "__present");
	let presence_id = inline_field_id(&inline.key, index, "__present");
	let presence_value = if row.id.is_some() { "true" } else { "false" }.to_owned();
	let presence = page!(|id: String, name: String, value: String| {
		input {
			type: "hidden",
			id: id,
			name: name,
			value: value,
		}
	})(presence_id, presence_name, presence_value);
	let Some(id) = &row.id else {
		return presence;
	};
	let id_name = inline_field_name(&inline.key, index, "__id");
	let id = id.clone();

	page!(|presence: Page, name: String, id: String| {
		{ presence }
		input {
			type: "hidden",
			name: name,
			value: id,
		}
	})(presence, id_name, id)
}

fn inline_delete_control(inline: &InlineFormInfo, row: &InlineRowInfo, index: usize) -> Page {
	if !inline.can_delete || row.id.is_none() {
		return Page::Empty;
	}
	let name = inline_field_name(&inline.key, index, "__delete");
	let input_id = inline_field_id(&inline.key, index, "__delete");
	let required_field_ids = inline
		.fields
		.iter()
		.filter(|field| {
			field.required
				&& !field.readonly
				&& inline.can_change
				&& !matches!(field.field_type, FieldType::Boolean)
		})
		.map(|field| inline_field_id(&inline.key, index, &field.name))
		.collect::<Vec<_>>();
	let label = format!("Delete {} {}", inline.model_name, index + 1);

	page!(|name: String, input_id: String, label: String, required_field_ids: Vec<String>| {
		label {
			class: "admin-inline-delete",
			input {
				id: input_id,
				type: "checkbox",
				name: name,
				aria_label: label.clone(),
				@change: move |event| {
					#[cfg(client)]
					if let Ok(deleted) = event.checked() {
						crate::pages::components::features::set_inline_required_fields(
							&required_field_ids,
							deleted,
						);
					}
				},
			}
			{ label }
		}
	})(name, input_id, label, required_field_ids)
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
	let value = row.values.get(&field.name).cloned().unwrap_or_default();
	let spec = crate::types::FormFieldSpec::from(&field.field_type);
	let kind = inline_value_kind(&spec, &value);
	inline_json_value_to_display_string(&normalized_inline_original(&value, kind))
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

fn model_form_page(
	model_name: &str,
	record_id: Option<&str>,
	form_groups: Page,
	prepopulated_fields: Vec<PrepopulatedField>,
	dirty_targets: HashSet<String>,
) -> Page {
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
	let history_links = record_id
		.map(|record_id| {
			Link::new(
				admin_record_url("history", model_name, record_id),
				"History",
			)
			.class("admin-btn admin-btn-secondary")
			.render()
		})
		.into_iter()
		.collect::<Vec<_>>();
	let submit_model = model_name.to_string();
	let submit_record_id = record_id.map(str::to_string);
	let submit_return_url = list_url.clone();
	let form_error = form_parent_error();
	let dirty_targets = Rc::new(RefCell::new(dirty_targets));

	page!(|form_title: String,
	 action_url: String,
	 form_groups: Page,
	 form_error: Page,
	 cancel_link: Page,
	 history_links: Vec<Page>,
	 submit_model: String,
	 submit_record_id: Option<String>,
	 submit_return_url: String,
	 prepopulated_fields: Vec<PrepopulatedField>,
	 dirty_targets: Rc<RefCell<HashSet<String>>>| {
		div {
			class: "model-form max-w-2xl animate__animated animate__fadeIn",
			h1 {
				class: "font-display text-2xl font-bold text-slate-900 mb-6",
				{ form_title }
			}
			form {
				method: "post",
				action: action_url,
				@input: move |event| {
					#[cfg(client)]
					crate::pages::components::features::handle_prepopulated_input(
						event.raw(),
						&prepopulated_fields,
						&dirty_targets,
					);
				},
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
				{ form_error }
				{ form_groups }
				div {
					class: "mt-6 flex gap-2",
					button {
						class: "admin-btn admin-btn-primary",
						type: "submit",
						"Save"
					}
					{ cancel_link }
					{ history_links }
				}
			}
		}
	})(
		form_title,
		action_url,
		form_groups,
		form_error,
		cancel_link,
		history_links,
		submit_model,
		submit_record_id,
		submit_return_url,
		prepopulated_fields,
		dirty_targets,
	)
}

#[cfg(client)]
fn handle_prepopulated_input(
	event: &web_sys::Event,
	rules: &[PrepopulatedField],
	dirty_targets: &Rc<RefCell<HashSet<String>>>,
) {
	use wasm_bindgen::JsCast;

	let Some(target) = event
		.target()
		.and_then(|target| target.dyn_into::<web_sys::Element>().ok())
	else {
		return;
	};
	let Some(name) = target.get_attribute("name") else {
		return;
	};
	let target_is_configured = rules.iter().any(|rule| rule.target == name);
	if target_is_configured {
		dirty_targets.borrow_mut().insert(name.clone());
	}
	if !rules
		.iter()
		.any(|rule| rule.sources.iter().any(|source| source == &name))
	{
		return;
	}

	let Some(form) = event
		.current_target()
		.or_else(|| event.target())
		.and_then(|target| target.dyn_into::<web_sys::HtmlFormElement>().ok())
	else {
		report_admin_error("Prepopulated form configuration could not find its parent form.");
		return;
	};
	let mut values = HashMap::new();
	for rule in rules {
		for field_name in rule.sources.iter().chain(std::iter::once(&rule.target)) {
			if values.contains_key(field_name) {
				continue;
			}
			let Some(control) = prepopulated_control(&form, field_name) else {
				report_admin_error(&format!(
					"Prepopulated form configuration could not find field '{field_name}'."
				));
				return;
			};
			let Some(value) = prepopulated_control_value(&control) else {
				report_admin_error(&format!(
					"Prepopulated form configuration field '{field_name}' has no text value."
				));
				return;
			};
			values.insert(field_name.clone(), value);
		}
	}

	for rule in rules {
		if dirty_targets.borrow().contains(&rule.target) {
			continue;
		}
		let source_values = rule
			.sources
			.iter()
			.map(|source| values.get(source).map(String::as_str).unwrap_or_default())
			.collect::<Vec<_>>()
			.join(" ");
		let value = reinhardt_utils::utils_core::encoding::slugify(&source_values);
		let Some(control) = prepopulated_control(&form, &rule.target) else {
			report_admin_error(&format!(
				"Prepopulated form configuration could not find field '{}'.",
				rule.target
			));
			return;
		};
		if !set_prepopulated_control_value(&control, &value) {
			report_admin_error(&format!(
				"Prepopulated form configuration field '{}' has no text value.",
				rule.target
			));
			return;
		}
		values.insert(rule.target.clone(), value);
	}
}

#[cfg(client)]
fn prepopulated_control(form: &web_sys::HtmlFormElement, name: &str) -> Option<web_sys::Element> {
	let elements = form.elements();
	(0..elements.length()).find_map(|index| {
		elements
			.item(index)
			.filter(|element| element.get_attribute("name").as_deref() == Some(name))
	})
}

#[cfg(client)]
fn prepopulated_control_value(element: &web_sys::Element) -> Option<String> {
	use wasm_bindgen::JsCast;

	if let Some(input) = element.dyn_ref::<web_sys::HtmlInputElement>() {
		return Some(input.value());
	}
	if let Some(textarea) = element.dyn_ref::<web_sys::HtmlTextAreaElement>() {
		return Some(textarea.value());
	}
	element
		.dyn_ref::<web_sys::HtmlSelectElement>()
		.map(web_sys::HtmlSelectElement::value)
}

#[cfg(client)]
fn set_prepopulated_control_value(element: &web_sys::Element, value: &str) -> bool {
	use wasm_bindgen::JsCast;

	if let Some(input) = element.dyn_ref::<web_sys::HtmlInputElement>() {
		input.set_value(value);
		return true;
	}
	if let Some(textarea) = element.dyn_ref::<web_sys::HtmlTextAreaElement>() {
		textarea.set_value(value);
		return true;
	}
	if let Some(select) = element.dyn_ref::<web_sys::HtmlSelectElement>() {
		select.set_value(value);
		return true;
	}
	false
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
		clear_validation_errors(form);
	}
	#[cfg(feature = "file-uploads")]
	let uses_multipart = form.as_ref().is_some_and(|form| {
		form.query_selector(r#"input[type="file"]"#)
			.ok()
			.flatten()
			.is_some()
	});
	#[cfg(not(feature = "file-uploads"))]
	let uses_multipart = false;
	let request = (!uses_multipart).then(|| collect_mutation_request(event.raw()));
	let form_for_submit = form.clone();
	reinhardt_pages::platform::spawn_task(async move {
		let result = if uses_multipart {
			let Some(form) = form_for_submit.as_ref() else {
				return report_admin_error("Save failed: form element is unavailable");
			};
			submit_model_form_multipart(form, &model_name, record_id.as_deref()).await
		} else if let Some(id) = record_id {
			update_record(
				model_name,
				id,
				request.expect("JSON form request must be collected"),
			)
			.await
		} else {
			create_record(
				model_name,
				request.expect("JSON form request must be collected"),
			)
			.await
		};

		match result {
			Ok(_) => navigate_or_set_href(&return_url),
			Err(error) => {
				let applied = form
					.as_ref()
					.is_some_and(|form| apply_validation_errors(form, &error));
				if !applied {
					report_admin_error(&format!("Save failed: {}", error));
				}
			}
		}
	});
}

#[cfg(client)]
async fn submit_model_form_multipart(
	form: &web_sys::HtmlFormElement,
	model_name: &str,
	record_id: Option<&str>,
) -> Result<MutationResponse, reinhardt_pages::server_fn::ServerFnError> {
	use wasm_bindgen::JsCast;

	let form_data = web_sys::FormData::new().map_err(|error| {
		reinhardt_pages::server_fn::ServerFnError::network(format!("{error:?}"))
	})?;
	append_json_form_part(
		&form_data,
		crate::server::multipart::MODEL_PART,
		&serde_json::Value::String(model_name.to_owned()),
	)?;
	if let Some(record_id) = record_id {
		append_json_form_part(
			&form_data,
			crate::server::multipart::ID_PART,
			&serde_json::Value::String(record_id.to_owned()),
		)?;
	}

	let elements = form.elements();
	for index in 0..elements.length() {
		let Some(element) = elements.item(index) else {
			continue;
		};
		if let Some(input) = element.dyn_ref::<web_sys::HtmlInputElement>() {
			let name = input.name();
			if name.is_empty() {
				continue;
			}
			if input.type_() == "file" {
				if let Some(files) = input.files()
					&& let Some(file) = files.item(0)
					&& (file.size() > 0.0 || !file.name().is_empty())
				{
					let blob: &web_sys::Blob = file.unchecked_ref();
					form_data.append_with_blob(&name, blob).map_err(|error| {
						reinhardt_pages::server_fn::ServerFnError::network(format!("{error:?}"))
					})?;
				}
				continue;
			}
			if name.starts_with(crate::server::multipart::CLEAR_PREFIX) && !input.checked() {
				continue;
			}
		}

		if let Some((name, value)) = form_control_name_value(&element) {
			append_json_form_part(&form_data, &name, &value)?;
		}
	}

	let path = if record_id.is_some() {
		"/api/server_fn/update_record_multipart"
	} else {
		"/api/server_fn/create_record_multipart"
	};
	let response = reinhardt_pages::server_fn::request_multipart(path, form_data, true).await?;
	if !response.is_success() {
		let status = response.status();
		let message = response.into_text();
		return Err(
			reinhardt_pages::server_fn::ServerFnError::from_http_response(status, &message),
		);
	}
	response.json()
}

#[cfg(client)]
fn append_json_form_part(
	form_data: &web_sys::FormData,
	name: &str,
	value: &serde_json::Value,
) -> Result<(), reinhardt_pages::server_fn::ServerFnError> {
	let value = serde_json::to_string(value).map_err(|error| {
		reinhardt_pages::server_fn::ServerFnError::serialization(error.to_string())
	})?;
	form_data
		.append_with_str(name, &value)
		.map_err(|error| reinhardt_pages::server_fn::ServerFnError::network(format!("{error:?}")))
}

#[cfg(client)]
fn clear_validation_errors(form: &web_sys::HtmlFormElement) {
	use wasm_bindgen::JsCast;

	if let Ok(errors) = form.query_selector_all(
		"[data-inline-row-error], [data-parent-field-error], [data-parent-form-error]",
	) {
		for index in 0..errors.length() {
			if let Some(error) = errors.item(index) {
				error.set_text_content(None);
			}
		}
	}

	if let Ok(fields) = form.query_selector_all("[aria-invalid=\"true\"]") {
		for index in 0..fields.length() {
			if let Some(field) = fields.item(index)
				&& let Ok(field) = field.dyn_into::<web_sys::Element>()
			{
				let _ = field.remove_attribute("aria-invalid");
				if let Some(described_by) = field.get_attribute("aria-describedby") {
					let described_by = described_by
						.split_whitespace()
						.filter(|id| !id.ends_with("-error") && !id.starts_with("inline-error-"))
						.collect::<Vec<_>>()
						.join(" ");
					if described_by.is_empty() {
						let _ = field.remove_attribute("aria-describedby");
					} else {
						let _ = field.set_attribute("aria-describedby", &described_by);
					}
				}
			}
		}
	}
}

#[cfg(client)]
fn apply_validation_errors(
	form: &web_sys::HtmlFormElement,
	error: &reinhardt_pages::server_fn::ServerFnError,
) -> bool {
	use reinhardt_pages::server_fn::ServerFnErrorKind;

	if error.kind() != ServerFnErrorKind::Validation {
		return false;
	}

	let mut messages_by_row: HashMap<String, Vec<String>> = HashMap::new();
	let mut messages_by_field: HashMap<String, Vec<String>> = HashMap::new();
	let mut global_messages = Vec::new();
	let mut applied = false;
	if error.field_errors().is_empty() {
		global_messages.push(error.to_string());
	}
	for field_error in error.field_errors() {
		let field_name = field_error.field();
		if let Some((key, index, field)) = parse_inline_error_path(field_name) {
			let row_error_id = inline_row_error_id(key, index);
			if let Ok(Some(_)) = form.query_selector(&format!("#{row_error_id}")) {
				if field == "_all" {
					messages_by_row
						.entry(row_error_id)
						.or_default()
						.push(field_error.message().to_string());
					applied = true;
					continue;
				}

				let field_id = inline_field_id(key, index, field);
				if let Ok(Some(input)) = form.query_selector(&format!("#{field_id}"))
					&& input.get_attribute("name").as_deref()
						== Some(inline_field_name(key, index, field).as_str())
				{
					let _ = input.set_attribute("aria-invalid", "true");
					let _ = input.set_attribute("aria-describedby", &row_error_id);
					messages_by_row
						.entry(row_error_id)
						.or_default()
						.push(field_error.message().to_string());
					applied = true;
					continue;
				}
			}
		}

		let field_id = format!("field-{field_name}");
		if let Some(input) = parent_validation_control(form, field_name)
			&& let Ok(Some(_)) = form.query_selector(&format!("#{field_id}-error"))
		{
			open_parent_fieldset(&input);
			let _ = input.set_attribute("aria-invalid", "true");
			let described_by = input.get_attribute("aria-describedby").unwrap_or_default();
			let error_id = format!("{field_id}-error");
			let _ = input.set_attribute(
				"aria-describedby",
				&append_described_by(&described_by, &error_id),
			);
			messages_by_field
				.entry(error_id)
				.or_default()
				.push(field_error.message().to_string());
			applied = true;
		} else {
			global_messages.push(field_error.message().to_string());
			applied = true;
		}
	}

	for (row_error_id, messages) in messages_by_row {
		if let Ok(Some(row_error)) = form.query_selector(&format!("#{row_error_id}")) {
			row_error.set_text_content(Some(&messages.join(" ")));
		}
	}
	for (error_id, messages) in messages_by_field {
		if let Ok(Some(error_node)) = form.query_selector(&format!("#{error_id}")) {
			error_node.set_text_content(Some(&messages.join(" ")));
		}
	}
	if !global_messages.is_empty()
		&& let Ok(Some(form_error)) = form.query_selector("[data-parent-form-error]")
	{
		form_error.set_text_content(Some(&global_messages.join(" ")));
	}

	applied || !global_messages.is_empty()
}

#[cfg(client)]
fn parent_validation_control(
	form: &web_sys::HtmlFormElement,
	field_name: &str,
) -> Option<web_sys::Element> {
	use wasm_bindgen::JsCast;

	let field_id = format!("field-{field_name}");
	if let Some(control) = [field_id.clone(), format!("{field_id}-search")]
		.into_iter()
		.find_map(|id| form.query_selector(&format!("#{id}")).ok().flatten())
	{
		return Some(control);
	}

	let Ok(controls) = form.query_selector_all("[data-parent-validation-control]") else {
		return None;
	};
	for index in 0..controls.length() {
		let Some(node) = controls.item(index) else {
			continue;
		};
		let Ok(control) = node.dyn_into::<web_sys::Element>() else {
			continue;
		};
		if control
			.get_attribute("data-parent-validation-control")
			.as_deref()
			== Some(field_name)
		{
			return Some(control);
		}
	}
	None
}

#[cfg(client)]
fn open_parent_fieldset(control: &web_sys::Element) {
	if let Ok(Some(details)) = control.closest("details") {
		let _ = details.set_attribute("open", "");
	}
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
			form_value_to_json(
				&name,
				&input.value(),
				input.type_() == "number",
				input.has_attribute("data-relation-id"),
			)
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
			let preserve_string_ids = element.has_attribute("data-relation-selector");
			let value = select_value_to_json(select, &name, preserve_string_ids);
			(name, value)
		});
	}

	None
}

#[cfg(client)]
fn select_value_to_json(
	select: &web_sys::HtmlSelectElement,
	name: &str,
	preserve_string_ids: bool,
) -> serde_json::Value {
	use wasm_bindgen::JsCast;

	if !select.multiple() {
		return form_value_to_json(name, &select.value(), false, false);
	}

	let options = select.options();
	let values: Vec<String> = (0..options.length())
		.filter_map(|index| {
			let option = options.item(index)?;
			let option = option.dyn_into::<web_sys::HtmlOptionElement>().ok()?;
			option.selected().then(|| option.value())
		})
		.collect();

	if preserve_string_ids {
		serde_json::Value::Array(values.into_iter().map(serde_json::Value::String).collect())
	} else {
		form_values_to_json_array(name, &values)
	}
}

#[cfg(any(client, test))]
fn form_values_to_json_array(name: &str, values: &[String]) -> serde_json::Value {
	serde_json::Value::Array(
		values
			.iter()
			.map(|value| form_value_to_json(name, value, false, false))
			.collect(),
	)
}

#[cfg(any(client, test))]
fn form_value_to_json(
	name: &str,
	value: &str,
	prefer_number: bool,
	relation_id: bool,
) -> serde_json::Value {
	if name.starts_with("__reinhardt_inlines.") {
		return serde_json::Value::String(value.to_string());
	}
	if relation_id {
		return if value.trim().is_empty() {
			serde_json::Value::Null
		} else {
			serde_json::Value::String(value.to_string())
		};
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

fn form_group_with_presentation(
	model_name: &str,
	field: &FormField,
	presentation: Option<&FieldPresentation>,
) -> Page {
	let input_id = format!("field-{}", field.name);
	let help_id = format!("{input_id}-help");
	let error_id = format!("{input_id}-error");
	let mut described_by = Vec::new();
	if presentation.is_some_and(|presentation| presentation.help_text.is_some()) {
		described_by.push(help_id.clone());
	}
	described_by.push(error_id.clone());
	let described_by = described_by.join(" ");
	let label = field.label.clone();
	let label_for = match &field.spec {
		crate::types::FormFieldSpec::Relation {
			widget: RelationWidget::Autocomplete,
			readonly: false,
			..
		} => format!("{input_id}-search"),
		_ => input_id.clone(),
	};
	let input = form_element_with_presentation_for_model(
		model_name,
		field,
		&input_id,
		&label,
		&described_by,
		presentation,
	);
	let help = presentation
		.and_then(|presentation| presentation.help_text.as_ref())
		.map(|help_text| {
			page!(|help_id: String, help_text: String| {
				div {
					id: help_id,
					class: "admin-help-text text-sm text-slate-500",
					{ help_text }
				}
			})(help_id.clone(), help_text.clone())
		});
	let help = help.unwrap_or(Page::Empty);
	let error = page!(|error_id: String| {
		div {
			id: error_id,
			class: "admin-field-error text-sm text-red-600",
			role: "alert",
			aria_live: "polite",
			data_parent_field_error: "true",
		}
	})(error_id);
	if matches!(
		&field.spec,
		crate::types::FormFieldSpec::ManyToManySelector { .. }
	) {
		return page!(|input: Page, help: Page, error: Page| {
			div {
				class: "mb-4",
				{ input }
				{ help }
				{ error }
			}
		})(input, help, error);
	}

	page!(|label_for: String, label: String, input: Page, help: Page, error: Page| {
		div {
			class: "mb-4",
			label {
				for: label_for,
				class: "admin-label",
				{ label }
			}
			{ input }
			{ help }
			{ error }
		}
	})(label_for, label, input, help, error)
}

fn form_parent_error() -> Page {
	page!(|| {
		div {
			class: "admin-form-error text-sm text-red-600",
			role: "alert",
			aria_live: "polite",
			data_parent_form_error: "true",
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

fn append_described_by(described_by: &str, extra_id: &str) -> String {
	let mut ids = described_by
		.split_whitespace()
		.map(str::to_string)
		.collect::<Vec<_>>();
	if !ids.iter().any(|id| id == extra_id) {
		ids.push(extra_id.to_string());
	}
	ids.join(" ")
}

#[allow(
	clippy::too_many_arguments,
	reason = "The renderer keeps independent relation field metadata explicit."
)]
fn render_relation(
	model_name: &str,
	field_name: &str,
	widget: RelationWidget,
	selected: Option<&RelationOption>,
	input_id: &str,
	name: String,
	label: String,
	value: String,
	placeholder: String,
	described_by: String,
	required: bool,
	readonly: bool,
) -> Page {
	if readonly {
		let display_value = selected
			.map(|option| option.label.clone())
			.filter(|label| !label.is_empty())
			.unwrap_or(value);
		return page!(|input_id: String, label: String, value: String| {
			span {
				id: input_id,
				class: "relation-readonly",
				aria_label: label,
				{ value }
			}
		})(input_id.to_string(), label, display_value);
	}

	match widget {
		RelationWidget::Autocomplete => render_autocomplete_relation(
			model_name,
			field_name,
			selected,
			input_id,
			name,
			label,
			value,
			placeholder,
			described_by,
			required,
		),
		RelationWidget::RawId => render_raw_id_relation(
			model_name,
			field_name,
			selected,
			input_id,
			name,
			label,
			value,
			placeholder,
			described_by,
			required,
		),
	}
}

#[cfg(client)]
fn update_relation_controls(
	search_id: &str,
	hidden_id: &str,
	search_value: &str,
	selected_value: &str,
	validation_message: &str,
) {
	use wasm_bindgen::JsCast;

	let Some(document) = web_sys::window().and_then(|window| window.document()) else {
		return;
	};
	if let Some(element) = document.get_element_by_id(search_id)
		&& let Ok(input) = element.dyn_into::<web_sys::HtmlInputElement>()
	{
		input.set_value(search_value);
		input.set_custom_validity(validation_message);
	}
	if let Some(element) = document.get_element_by_id(hidden_id)
		&& let Ok(input) = element.dyn_into::<web_sys::HtmlInputElement>()
	{
		input.set_value(selected_value);
	}
}

#[cfg(client)]
const RELATION_QUERY_DEBOUNCE_MS: i32 = 150;

#[cfg(client)]
fn schedule_relation_query(
	debounced_query: Signal<String>,
	debounce_generation: Rc<Cell<u64>>,
	value: String,
) {
	use wasm_bindgen::JsCast;

	let generation = debounce_generation.get().wrapping_add(1);
	debounce_generation.set(generation);
	let fallback_value = value.clone();
	let callback_query = debounced_query.clone();
	let callback_generation = debounce_generation.clone();
	let callback = wasm_bindgen::closure::Closure::once_into_js(move || {
		if callback_generation.get() == generation {
			callback_query.set(value);
		}
	});
	let Some(window) = web_sys::window() else {
		debounced_query.set(fallback_value);
		return;
	};
	if window
		.set_timeout_with_callback_and_timeout_and_arguments_0(
			callback.unchecked_ref(),
			RELATION_QUERY_DEBOUNCE_MS,
		)
		.is_err()
	{
		debounced_query.set(fallback_value);
	}
}

#[cfg(client)]
#[allow(
	clippy::too_many_arguments,
	reason = "The renderer keeps independent relation field metadata explicit."
)]
fn render_raw_id_relation(
	model_name: &str,
	field_name: &str,
	selected: Option<&RelationOption>,
	input_id: &str,
	name: String,
	label: String,
	value: String,
	placeholder: String,
	described_by: String,
	required: bool,
) -> Page {
	let resolved_label = Signal::new(
		selected
			.map(|option| option.label.clone())
			.unwrap_or_default(),
	);
	let status = Signal::new(String::new());
	let generation = Rc::new(Cell::new(0_u64));
	let model_name = model_name.to_string();
	let field_name = field_name.to_string();
	let input_id = input_id.to_string();
	let status_id = format!("{input_id}-status");
	let aria_describedby = append_described_by(&described_by, &status_id);
	let label_view = Page::reactive({
		let resolved_label = resolved_label.clone();
		let status = status.clone();
		let status_id = status_id.clone();
		move || {
			let resolved_label = resolved_label.get();
			let status = status.get();
			let status_text = if status.is_empty() {
				resolved_label
			} else {
				format!("{resolved_label} {status}")
			};
			page!(|status_id: String, status_text: String| {
				div {
					class: "relation-resolved-label",
					span {
						id: status_id,
						role: "status",
						aria_live: "polite",
						{ status_text }
					}
				}
			})(status_id.clone(), status_text)
		}
	});
	if required {
		page!(|input_id: String,
		 name: String,
		 input_label: String,
			 aria_describedby: String,
			 value: String,
			 placeholder: String,
			 label_view: Page,
		 resolved_label: Signal<String>,
		 status: Signal<String>,
		 generation: Rc<Cell<u64>>,
		 model_name: String,
		 field_name: String| {
			div {
				class: "relation-raw-id",
				input {
					class: "admin-input",
					type: "text",
					id: input_id,
					name: name,
					data_relation_id: "true",
					aria_label: input_label,
					aria_describedby: aria_describedby,
					value: value,
					placeholder: placeholder,
					required: true,
					autocomplete: "off",
					@change: move |event| {
						crate::pages::components::features::resolve_raw_relation(
							model_name.clone(),
							field_name.clone(),
							event.value().unwrap_or_default(),
							resolved_label,
							status,
							generation.clone(),
						)
					},
				}
				{ label_view }
			}
		})(
			input_id,
			name,
			label,
			aria_describedby,
			value,
			placeholder,
			label_view,
			resolved_label,
			status,
			generation,
			model_name,
			field_name,
		)
	} else {
		page!(|input_id: String,
		 name: String,
		 input_label: String,
			 aria_describedby: String,
			 value: String,
			 placeholder: String,
			 label_view: Page,
		 resolved_label: Signal<String>,
		 status: Signal<String>,
		 generation: Rc<Cell<u64>>,
		 model_name: String,
		 field_name: String| {
			div {
				class: "relation-raw-id",
				input {
					class: "admin-input",
					type: "text",
					id: input_id,
					name: name,
					data_relation_id: "true",
					aria_label: input_label,
					aria_describedby: aria_describedby,
					value: value,
					placeholder: placeholder,
					autocomplete: "off",
					@change: move |event| {
						crate::pages::components::features::resolve_raw_relation(
							model_name.clone(),
							field_name.clone(),
							event.value().unwrap_or_default(),
							resolved_label,
							status,
							generation.clone(),
						)
					},
				}
				{ label_view }
			}
		})(
			input_id,
			name,
			label,
			aria_describedby,
			value,
			placeholder,
			label_view,
			resolved_label,
			status,
			generation,
			model_name,
			field_name,
		)
	}
}

#[cfg(client)]
fn resolve_raw_relation(
	model_name: String,
	field_name: String,
	id: String,
	resolved_label: Signal<String>,
	status: Signal<String>,
	generation: Rc<Cell<u64>>,
) {
	let current_generation = generation.get().wrapping_add(1);
	generation.set(current_generation);
	if id.trim().is_empty() {
		let _ = resolved_label.try_set(String::new());
		let _ = status.try_set(String::new());
		return;
	}
	let _ = status.try_set("Resolving…".to_string());
	reinhardt_pages::platform::spawn_task(async move {
		match get_relation_options(
			model_name,
			field_name,
			RelationLookupRequest::Resolve { id },
		)
		.await
		{
			Ok(response) if generation.get() == current_generation => {
				let _ = resolved_label.try_set(
					response
						.results
						.first()
						.map(|option| option.label.clone())
						.unwrap_or_default(),
				);
				let _ = status.try_set(String::new());
			}
			Err(error) if generation.get() == current_generation => {
				let _ = resolved_label.try_set(String::new());
				let _ = status.try_set(format!("Unable to resolve relation: {error}"));
			}
			_ => {}
		}
	});
}

#[cfg(not(client))]
#[allow(
	clippy::too_many_arguments,
	reason = "The renderer keeps independent relation field metadata explicit."
)]
fn render_raw_id_relation(
	_model_name: &str,
	_field_name: &str,
	selected: Option<&RelationOption>,
	input_id: &str,
	name: String,
	label: String,
	value: String,
	placeholder: String,
	described_by: String,
	required: bool,
) -> Page {
	let resolved_label = selected
		.map(|option| option.label.clone())
		.unwrap_or_default();
	let status_id = format!("{input_id}-status");
	let aria_describedby = append_described_by(&described_by, &status_id);
	if required {
		page!(|input_id: String,
		 name: String,
		 label: String,
			 status_id: String,
			 aria_describedby: String,
			 value: String,
			 placeholder: String,
			 resolved_label: String| {
			div {
				class: "relation-raw-id",
				input {
					class: "admin-input",
					type: "text",
					id: input_id,
					name: name,
					data_relation_id: "true",
					aria_label: label,
					aria_describedby: aria_describedby,
					value: value,
					placeholder: placeholder,
					required: true,
					autocomplete: "off",
				}
				span {
					id: status_id,
					role: "status",
					aria_live: "polite",
					{ resolved_label }
				}
			}
		})(
			input_id.to_string(),
			name,
			label,
			status_id,
			aria_describedby,
			value,
			placeholder,
			resolved_label,
		)
	} else {
		page!(|input_id: String,
		 name: String,
		 label: String,
			 status_id: String,
			 aria_describedby: String,
			 value: String,
			 placeholder: String,
			 resolved_label: String| {
			div {
				class: "relation-raw-id",
				input {
					class: "admin-input",
					type: "text",
					id: input_id,
					name: name,
					data_relation_id: "true",
					aria_label: label,
					aria_describedby: aria_describedby,
					value: value,
					placeholder: placeholder,
					autocomplete: "off",
				}
				span {
					id: status_id,
					role: "status",
					aria_live: "polite",
					{ resolved_label }
				}
			}
		})(
			input_id.to_string(),
			name,
			label,
			status_id,
			aria_describedby,
			value,
			placeholder,
			resolved_label,
		)
	}
}

#[cfg(client)]
#[allow(
	clippy::too_many_arguments,
	reason = "The renderer keeps independent relation field metadata explicit."
)]
fn render_autocomplete_relation(
	model_name: &str,
	field_name: &str,
	selected: Option<&RelationOption>,
	input_id: &str,
	name: String,
	label: String,
	value: String,
	placeholder: String,
	described_by: String,
	required: bool,
) -> Page {
	let query = Signal::new(
		selected
			.map(|option| option.label.clone())
			.unwrap_or_default(),
	);
	let debounced_query = Signal::new(String::new());
	let selected_id = Signal::new(value.clone());
	let page_signal = Signal::new(1_u64);
	let debounce_generation = Rc::new(Cell::new(0_u64));
	let model_name = model_name.to_string();
	let field_name = field_name.to_string();
	let resource = reinhardt_pages::use_resource(
		{
			let debounced_query = debounced_query.clone();
			let page_signal = page_signal.clone();
			let model_name = model_name.clone();
			let field_name = field_name.clone();
			move || {
				let request = RelationLookupRequest::Search {
					query: debounced_query.get(),
					page: Some(page_signal.get()),
					page_size: Some(20),
				};
				let model_name = model_name.clone();
				let field_name = field_name.clone();
				async move {
					get_relation_options(model_name, field_name, request)
						.await
						.map_err(|error| error.to_string())
				}
			}
		},
		reinhardt_pages::deps![debounced_query, page_signal],
	);
	let input_id = input_id.to_string();
	let search_id = format!("{input_id}-search");
	let hidden_id = format!("{search_id}-value");
	let list_id = format!("{input_id}-options");
	let status_id = format!("{input_id}-status");
	let aria_describedby = append_described_by(&described_by, &status_id);
	let input_label = label.clone();
	let reactive_content = Page::reactive({
		let resource = resource.clone();
		let query = query.clone();
		let selected_id = selected_id.clone();
		let page_signal = page_signal.clone();
		let search_id = search_id.clone();
		let hidden_id = hidden_id.clone();
		let list_id = list_id.clone();
		let status_id = status_id.clone();
		move || {
			let state = resource.get();
			let (results, page, has_next, status) = match state {
				ResourceState::Loading => {
					(Vec::new(), page_signal.get(), false, "Loading…".to_string())
				}
				ResourceState::Success(response) => (
					response.results,
					response.page,
					response.has_next,
					String::new(),
				),
				ResourceState::Error(error) => (
					Vec::new(),
					page_signal.get(),
					false,
					format!("Unable to load relation options: {error}"),
				),
			};
			let option_pages = relation_option_pages(
				results,
				selected_id,
				query,
				search_id.clone(),
				hidden_id.clone(),
			);
			let status_page = page!(|status_id: String, status: String| {
				span {
					id: status_id,
					role: "status",
					aria_live: "polite",
					{ status }
				}
			})(status_id.clone(), status);
			let pagination = page!(|page: u64, has_next: bool, page_signal: Signal<u64>| {
				div {
					class: "relation-pagination",
					button {
						type: "button",
						disabled: page <= 1,
						@click: {
							let page_signal = page_signal.clone();
							move |_| page_signal.set(page.saturating_sub(1).max(1))
						},
						"Previous"
					}
					span {
						aria_live: "polite",
						{ format!("Page {page}") }
					}
					button {
						type: "button",
						disabled: !has_next,
						@click: {
							let page_signal = page_signal.clone();
							move |_| page_signal.set(page.saturating_add(1))
						},
						"Next"
					}
				}
			})(page, has_next, page_signal);
			page!(|list_id: String, option_pages: Vec<Page>, status_page: Page, pagination: Page| {
				div {
					div {
						id: list_id,
						role: "listbox",
						{ option_pages }
					}
					{ status_page }
					{ pagination }
				}
			})(list_id.clone(), option_pages, status_page, pagination)
		}
	});
	let search_input = if required {
		page!(|search_id: String,
		 input_label: String,
		 list_id: String,
		 aria_describedby: String,
		 query: Signal<String>,
		 selected_id: Signal<String>,
		 page_signal: Signal<u64>,
			 debounced_query: Signal<String>,
			 debounce_generation: Rc<Cell<u64>>,
			 hidden_id: String,
			 placeholder: String| {
			input {
				class: "admin-input",
				type: "search",
				id: search_id.clone(),
				aria_label: input_label,
				role: "combobox",
				aria_controls: list_id,
				aria_expanded: "true",
					aria_describedby: aria_describedby,
					value: query.get(),
					placeholder: placeholder,
					autocomplete: "off",
				required: true,
				@input: move |event| {
					if event.is_composing() {
						return;
					}
					let value = event.value().unwrap_or_default();
					query.set(value.clone());
					selected_id.set(String::new());
					crate::pages::components::features::schedule_relation_query(
						debounced_query.clone(),
						debounce_generation.clone(),
						value.clone(),
					);
					crate::pages::components::features::update_relation_controls(
						&search_id,
						&hidden_id,
						&value,
						"",
						"Select a relation.",
					);
					page_signal.set(1);
				},
			}
		})(
			search_id.clone(),
			input_label.clone(),
			list_id.clone(),
			aria_describedby.clone(),
			query.clone(),
			selected_id.clone(),
			page_signal.clone(),
			debounced_query.clone(),
			debounce_generation.clone(),
			hidden_id.clone(),
			placeholder.clone(),
		)
	} else {
		page!(|search_id: String,
		 input_label: String,
		 list_id: String,
		 aria_describedby: String,
		 query: Signal<String>,
		 selected_id: Signal<String>,
		 page_signal: Signal<u64>,
			 debounced_query: Signal<String>,
			 debounce_generation: Rc<Cell<u64>>,
			 hidden_id: String,
			 placeholder: String| {
			input {
				class: "admin-input",
				type: "search",
				id: search_id.clone(),
				aria_label: input_label,
				role: "combobox",
				aria_controls: list_id,
				aria_expanded: "true",
					aria_describedby: aria_describedby,
					value: query.get(),
					placeholder: placeholder,
					autocomplete: "off",
				@input: move |event| {
					if event.is_composing() {
						return;
					}
					let value = event.value().unwrap_or_default();
					query.set(value.clone());
					selected_id.set(String::new());
					crate::pages::components::features::schedule_relation_query(
						debounced_query.clone(),
						debounce_generation.clone(),
						value.clone(),
					);
					crate::pages::components::features::update_relation_controls(
						&search_id, &hidden_id, &value, "", "",
					);
					page_signal.set(1);
				},
			}
		})(
			search_id.clone(),
			input_label,
			list_id.clone(),
			aria_describedby,
			query.clone(),
			selected_id.clone(),
			page_signal.clone(),
			debounced_query,
			debounce_generation,
			hidden_id.clone(),
			placeholder,
		)
	};
	let hidden_input = if required {
		page!(|hidden_id: String, name: String, selected_value: String| {
			input {
				type: "hidden",
				id: hidden_id,
				name: name,
				data_relation_id: "true",
				value: selected_value,
				required: true,
			}
		})(hidden_id, name, value)
	} else {
		page!(|hidden_id: String, name: String, selected_value: String| {
			input {
				type: "hidden",
				id: hidden_id,
				name: name,
				data_relation_id: "true",
				value: selected_value,
			}
		})(hidden_id, name, value)
	};
	page!(|search_input: Page, hidden_input: Page, reactive_content: Page| {
		div {
			class: "relation-autocomplete",
			{ search_input }
			{ hidden_input }
			{ reactive_content }
		}
	})(search_input, hidden_input, reactive_content)
}

#[cfg(not(client))]
#[allow(
	clippy::too_many_arguments,
	reason = "The renderer keeps independent relation field metadata explicit."
)]
fn render_autocomplete_relation(
	_model_name: &str,
	_field_name: &str,
	selected: Option<&RelationOption>,
	input_id: &str,
	name: String,
	label: String,
	value: String,
	placeholder: String,
	described_by: String,
	required: bool,
) -> Page {
	let search_id = format!("{input_id}-search");
	let list_id = format!("{input_id}-options");
	let status_id = format!("{input_id}-status");
	let aria_describedby = append_described_by(&described_by, &status_id);
	let query = selected
		.map(|option| option.label.clone())
		.unwrap_or_default();
	if required {
		page!(|search_id: String,
		 list_id: String,
		 status_id: String,
		 aria_describedby: String,
			 label: String,
			 query: String,
			 name: String,
			 value: String,
			 placeholder: String| {
			div {
				class: "relation-autocomplete",
				input {
					class: "admin-input",
					type: "search",
					id: search_id,
					aria_label: label,
					role: "combobox",
					aria_controls: list_id.clone(),
					aria_expanded: "true",
						aria_describedby: aria_describedby,
						value: query,
						placeholder: placeholder,
						autocomplete: "off",
					required: true,
				}
				input {
					type: "hidden",
					name: name,
					data_relation_id: "true",
					value: value,
					required: true
				}
				div {
					id: list_id,
					role: "listbox"
				}
				span {
					id: status_id,
					role: "status",
					aria_live: "polite",
					""
				}
			}
		})(
			search_id,
			list_id,
			status_id,
			aria_describedby,
			label,
			query,
			name,
			value,
			placeholder,
		)
	} else {
		page!(|search_id: String,
		 list_id: String,
		 status_id: String,
		 aria_describedby: String,
			 label: String,
			 query: String,
			 name: String,
			 value: String,
			 placeholder: String| {
			div {
				class: "relation-autocomplete",
				input {
					class: "admin-input",
					type: "search",
					id: search_id,
					aria_label: label,
					role: "combobox",
					aria_controls: list_id.clone(),
					aria_expanded: "true",
						aria_describedby: aria_describedby,
						value: query,
						placeholder: placeholder,
						autocomplete: "off",
				}
				input {
					type: "hidden",
					name: name,
					data_relation_id: "true",
					value: value
				}
				div {
					id: list_id,
					role: "listbox"
				}
				span {
					id: status_id,
					role: "status",
					aria_live: "polite",
					""
				}
			}
		})(
			search_id,
			list_id,
			status_id,
			aria_describedby,
			label,
			query,
			name,
			value,
			placeholder,
		)
	}
}

#[cfg(client)]
fn relation_option_pages(
	options: Vec<RelationOption>,
	selected_id: Signal<String>,
	query: Signal<String>,
	search_id: String,
	hidden_id: String,
) -> Vec<Page> {
	options
		.into_iter()
		.map(|option| {
			let id = option.id.clone();
			let label = option.label.clone();
			let selected = selected_id.get() == id;
			let selected_id = selected_id.clone();
			let query = query.clone();
			let search_id = search_id.clone();
			let hidden_id = hidden_id.clone();
			page!(|id: String,
			 label: String,
			 display_label: String,
			 selected: bool,
			 selected_id: Signal<String>,
			 query: Signal<String>,
			 search_id: String,
			 hidden_id: String| {
				button {
					type: "button",
					role: "option",
					aria_selected: if selected { "true" } else { "false" },
					value: id.clone(),
					@click: move |_| {
						selected_id.set(id.clone());
						query.set(label.clone());
						crate::pages::components::features::update_relation_controls(
							&search_id, &hidden_id, &label, &id, "",
						);
					},
					{ display_label }
				}
			})(
				id,
				label.clone(),
				label,
				selected,
				selected_id,
				query,
				search_id,
				hidden_id,
			)
		})
		.collect()
}

#[cfg(test)]
fn form_element_with_description_for_model(
	model_name: &str,
	field: &FormField,
	input_id: &str,
	label: &str,
	described_by: &str,
) -> Page {
	form_element_with_presentation_and_step(
		model_name,
		field,
		input_id,
		label,
		described_by,
		None,
		None,
	)
}

fn form_element_with_presentation_for_model(
	model_name: &str,
	field: &FormField,
	input_id: &str,
	label: &str,
	described_by: &str,
	presentation: Option<&FieldPresentation>,
) -> Page {
	form_element_with_presentation_and_step(
		model_name,
		field,
		input_id,
		label,
		described_by,
		presentation,
		None,
	)
}

fn form_element_with_description_for_model_and_step(
	model_name: &str,
	field: &FormField,
	input_id: &str,
	label: &str,
	described_by: &str,
	step: Option<&str>,
) -> Page {
	form_element_with_presentation_and_step(
		model_name,
		field,
		input_id,
		label,
		described_by,
		None,
		step,
	)
}

#[allow(
	clippy::too_many_arguments,
	reason = "The renderer keeps independent presentation and input metadata explicit."
)]
fn form_element_with_presentation_and_step(
	model_name: &str,
	field: &FormField,
	input_id: &str,
	label: &str,
	described_by: &str,
	presentation: Option<&FieldPresentation>,
	step: Option<&str>,
) -> Page {
	use crate::types::FormFieldSpec;

	let input_id = input_id.to_string();
	let name = field.name.clone();
	let label = label.to_string();
	let described_by = described_by.to_string();
	let value = field.value.clone();
	let required = field.required;
	let placeholder = presentation
		.and_then(|presentation| presentation.placeholder.clone())
		.unwrap_or_default();

	match &field.spec {
		FormFieldSpec::Input { html_type } => render_input(
			html_type.clone(),
			input_id,
			name,
			label,
			described_by,
			value,
			placeholder,
			required,
			step.map(str::to_string),
		),
		FormFieldSpec::File => render_file_input(
			input_id,
			name,
			label,
			described_by,
			required && value.is_empty(),
			field.nullable && !value.is_empty(),
		),
		FormFieldSpec::Hidden => render_input(
			"hidden".to_string(),
			input_id,
			name,
			label,
			described_by,
			value,
			placeholder,
			required,
			None,
		),
		FormFieldSpec::Relation {
			field_name,
			widget,
			selected,
			readonly,
		} => render_relation(
			model_name,
			field_name,
			*widget,
			selected.as_ref(),
			&input_id,
			name,
			label,
			value,
			placeholder,
			described_by,
			required,
			*readonly,
		),
		FormFieldSpec::TextArea | FormFieldSpec::Json => render_textarea(
			input_id,
			name,
			label,
			described_by,
			value,
			placeholder,
			required,
			None,
		),
		FormFieldSpec::TextAreaWithRows { rows } => render_textarea(
			input_id,
			name,
			label,
			described_by,
			value,
			placeholder,
			required,
			*rows,
		),
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
		FormFieldSpec::ManyToManySelector {
			layout,
			available,
			selected,
			has_more,
		} => crate::pages::components::relation_selector::relation_selector_with_description(
			model_name,
			&name,
			&label,
			*layout,
			available.clone(),
			selected.clone(),
			*has_more,
			&described_by,
		),
	}
}

#[allow(
	clippy::too_many_arguments,
	reason = "The renderer keeps independent textarea accessibility metadata explicit."
)]
fn render_textarea(
	input_id: String,
	name: String,
	label: String,
	described_by: String,
	value: String,
	placeholder: String,
	required: bool,
	rows: Option<u16>,
) -> Page {
	let rows = rows.unwrap_or(2).to_string();
	if required {
		page!(|input_id: String,
		 name: String,
		 label: String,
		 described_by: String,
		 value: String,
		 placeholder: String,
		 rows: String| {
			textarea {
				class: "admin-input",
				id: input_id,
				name: name,
				rows: rows,
				aria_label: label,
				aria_describedby: described_by,
				placeholder: placeholder,
				required: true,
				autocomplete: "off",
				{ value }
			}
		})(
			input_id,
			name,
			label,
			described_by,
			value,
			placeholder,
			rows,
		)
	} else {
		page!(|input_id: String,
		 name: String,
		 label: String,
		 described_by: String,
		 value: String,
		 placeholder: String,
		 rows: String| {
			textarea {
				class: "admin-input",
				id: input_id,
				name: name,
				rows: rows,
				aria_label: label,
				aria_describedby: described_by,
				placeholder: placeholder,
				autocomplete: "off",
				{ value }
			}
		})(
			input_id,
			name,
			label,
			described_by,
			value,
			placeholder,
			rows,
		)
	}
}

fn render_file_input(
	input_id: String,
	name: String,
	label: String,
	described_by: String,
	required: bool,
	show_clear: bool,
) -> Page {
	let clear_id = format!("{input_id}-clear");
	let clear_name = format!("{}{}", crate::server::multipart::CLEAR_PREFIX, name);
	if show_clear {
		page!(|input_id: String,
		 name: String,
		 label: String,
		 described_by: String,
		 clear_id: String,
		 clear_name: String,
		 required: bool| {
			div {
				input {
					class: "admin-input",
					type: "file",
					id: input_id,
					name: name,
					aria_label: label,
					aria_describedby: described_by,
					required: required,
					autocomplete: "off",
				}
				label {
					class: "admin-checkbox-label mt-2",
					for: clear_id.clone(),
					input {
						class: "admin-input",
						type: "checkbox",
						id: clear_id,
						name: clear_name,
						value: "true",
					}"Clear current file"
				}
			}
		})(
			input_id,
			name,
			label,
			described_by,
			clear_id,
			clear_name,
			required,
		)
	} else {
		page!(|input_id: String, name: String, label: String, described_by: String, required: bool| {
			input {
				class: "admin-input",
				type: "file",
				id: input_id,
				name: name,
				aria_label: label,
				aria_describedby: described_by,
				required: required,
				autocomplete: "off",
			}
		})(input_id, name, label, described_by, required)
	}
}

/// Render an `<input>` element with the given HTML `type`.
#[allow(
	clippy::too_many_arguments,
	reason = "The renderer keeps independent input accessibility metadata explicit."
)]
fn render_input(
	html_type: String,
	input_id: String,
	name: String,
	label: String,
	described_by: String,
	value: String,
	placeholder: String,
	required: bool,
	step: Option<String>,
) -> Page {
	if html_type == "checkbox" {
		let checked = matches!(value.as_str(), "true" | "1" | "on");
		return render_checkbox_input(
			input_id,
			name,
			label,
			described_by,
			value,
			placeholder,
			checked,
		);
	}
	if html_type == "number" {
		let step = step.unwrap_or_else(|| "any".to_string());
		return page!(|input_id: String,
		 name: String,
		 label: String,
		 described_by: String,
		 value: String,
		 placeholder: String,
		 step: String,
		 required: bool| {
			input {
				class: "admin-input",
				type: "number",
				id: input_id,
				name: name,
				aria_label: label,
				aria_describedby: described_by,
				value: value,
				placeholder: placeholder,
				step: step,
				required: required,
				autocomplete: "off",
			}
		})(
			input_id,
			name,
			label,
			described_by,
			value,
			placeholder,
			step,
			required,
		);
	}
	if matches!(html_type.as_str(), "time" | "datetime-local") {
		return page!(|html_type: String,
		 input_id: String,
		 name: String,
		 label: String,
		 described_by: String,
		 value: String,
		 placeholder: String,
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
				placeholder: placeholder,
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
			placeholder,
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
		 value: String,
		 placeholder: String| {
			input {
				class: "admin-input",
				type: html_type,
				id: input_id,
				name: name,
				aria_label: label,
				aria_describedby: described_by,
				value: value,
				placeholder: placeholder,
				required: true,
				autocomplete: "off",
			}
		})(
			html_type,
			input_id,
			name,
			label,
			described_by,
			value,
			placeholder,
		)
	} else {
		page!(|html_type: String,
		 input_id: String,
		 name: String,
		 label: String,
		 described_by: String,
		 value: String,
		 placeholder: String| {
			input {
				class: "admin-input",
				type: html_type,
				id: input_id,
				name: name,
				aria_label: label,
				aria_describedby: described_by,
				value: value,
				placeholder: placeholder,
				autocomplete: "off",
			}
		})(
			html_type,
			input_id,
			name,
			label,
			described_by,
			value,
			placeholder,
		)
	}
}

fn render_checkbox_input(
	input_id: String,
	name: String,
	label: String,
	described_by: String,
	value: String,
	placeholder: String,
	checked: bool,
) -> Page {
	page!(|input_id: String,
	 name: String,
	 label: String,
	 described_by: String,
	 value: String,
	 placeholder: String,
	 checked: bool| {
		input {
			class: "admin-input",
			type: "checkbox",
			id: input_id,
			name: name,
			aria_label: label,
			aria_describedby: described_by,
			value: value,
			placeholder: placeholder,
			checked: checked,
			autocomplete: "off",
		}
	})(
		input_id,
		name,
		label,
		described_by,
		value,
		placeholder,
		checked,
	)
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
		AdminAction, Column, FieldPresentation, FormField, InlineControlSnapshot, InlineValueKind,
		ListViewData, action_can_dispatch, admin_record_url, apply_prepopulated_values, data_table,
		decode_admin_path_segment, detail_table, detail_view, find_admin_action,
		form_element_with_description_for_model, form_group_with_presentation, form_value_to_json,
		form_values_to_json_array, history_view, html_id_segment, inline_edit_request,
		inline_edit_updates, inline_error_message, inline_scalar_value, inline_value_kind,
		apply_date_hierarchy_choice, list_view, list_view_with_actions,
		list_view_with_date_hierarchy, model_form,
		normalized_inline_original, nullable_boolean_choices, record_primary_key, scalar_object_id,
		set_page_selected, set_record_selected, tagged_inline_json_value,
	};
	use crate::types::{
		AdminActionRequest, AdminHistoryEntry, DateHierarchyInfo, DateHierarchyLevel,
		DateHierarchyListQueryParams, DateHierarchySelection, FormFieldSpec, HistoryResponse,
		InlineEditError, InlineEditResponse, ListQueryParams, ModelPermission, MutationResponse,
		PrepopulatedField, RelationOption, RelationWidget,
	};
	use reinhardt_core::reactive::ReactiveScope;
	use reinhardt_pages::Signal;
	use reinhardt_pages::reactive::use_action;
	use reinhardt_pages::testing::component::render;
	use rstest::rstest;
	use serde_json::json;
	use std::cell::RefCell;
	use std::collections::{BTreeSet, HashMap, HashSet};
	use std::rc::Rc;

	#[rstest]
	fn prepopulated_values_slugify_one_source() {
		let mut values = HashMap::from([(String::from("title"), String::from("Hello World"))]);
		let rules = vec![PrepopulatedField::new("slug", ["title"])];

		apply_prepopulated_values(&mut values, &rules, &HashSet::new());

		assert_eq!(values.get("slug"), Some(&String::from("hello-world")));
	}

	#[rstest]
	fn prepopulated_values_join_sources_in_declaration_order() {
		let mut values = HashMap::from([
			(String::from("title"), String::from("Hello World")),
			(String::from("category"), String::from("News")),
		]);
		let rules = vec![PrepopulatedField::new("slug", ["title", "category"])];

		apply_prepopulated_values(&mut values, &rules, &HashSet::new());

		assert_eq!(values.get("slug"), Some(&String::from("hello-world-news")));
	}

	#[rstest]
	fn prepopulated_values_resolve_chains_in_rule_order() {
		let mut values = HashMap::from([(String::from("title"), String::from("Hello World"))]);
		let rules = vec![
			PrepopulatedField::new("slug", ["title"]),
			PrepopulatedField::new("seo_slug", ["slug"]),
		];

		apply_prepopulated_values(&mut values, &rules, &HashSet::new());

		assert_eq!(values.get("slug"), Some(&String::from("hello-world")));
		assert_eq!(values.get("seo_slug"), Some(&String::from("hello-world")));
	}

	#[rstest]
	fn prepopulated_values_preserve_non_empty_dirty_target() {
		let mut values = HashMap::from([
			(String::from("title"), String::from("Hello World")),
			(String::from("slug"), String::from("existing-slug")),
		]);
		let rules = vec![PrepopulatedField::new("slug", ["title"])];
		let dirty_targets = HashSet::from([String::from("slug")]);

		apply_prepopulated_values(&mut values, &rules, &dirty_targets);

		assert_eq!(values.get("slug"), Some(&String::from("existing-slug")));
	}

	#[rstest]
	fn prepopulated_values_keep_cleared_manual_target_dirty() {
		let mut values = HashMap::from([
			(String::from("title"), String::from("Hello World")),
			(String::from("slug"), String::new()),
		]);
		let rules = vec![PrepopulatedField::new("slug", ["title"])];
		let dirty_targets = HashSet::from([String::from("slug")]);

		apply_prepopulated_values(&mut values, &rules, &dirty_targets);

		assert_eq!(values.get("slug"), Some(&String::new()));
	}

	#[rstest]
	fn prepopulated_values_do_not_mark_automatic_targets_dirty() {
		let mut values = HashMap::from([(String::from("title"), String::from("Hello World"))]);
		let rules = vec![PrepopulatedField::new("slug", ["title"])];
		let dirty_targets = HashSet::new();

		apply_prepopulated_values(&mut values, &rules, &dirty_targets);

		assert!(dirty_targets.is_empty());
		assert_eq!(values.get("slug"), Some(&String::from("hello-world")));
	}

	#[rstest]
	fn admin_action_primary_key_uses_the_configured_field_without_an_id_fallback() {
		// Arrange
		let record = HashMap::from([
			("id".to_string(), json!("17")),
			("slug".to_string(), json!("release-notes")),
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
				editable: false,
				linked: false,
				required: false,
				nullable: false,
				step: None,
				form_spec: None,
			}],
			pk_field: "slug".to_string(),
			records: vec![
				HashMap::from([
					("id".to_string(), json!(41)),
					("slug".to_string(), json!("article-a")),
					("title".to_string(), json!("Article A")),
				]),
				HashMap::from([
					("id".to_string(), json!(42)),
					("slug".to_string(), json!("article-b")),
					("title".to_string(), json!("Article B")),
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

	fn list_data(records: Vec<HashMap<String, serde_json::Value>>) -> ListViewData {
		ListViewData {
			model_name: "Article".to_string(),
			columns: vec![Column {
				field: "summary".to_string(),
				label: "Summary".to_string(),
				sortable: false,
				editable: false,
				linked: false,
				required: false,
				nullable: false,
				step: None,
				form_spec: None,
			}],
			pk_field: "id".to_string(),
			records,
			current_page: 1,
			total_pages: 1,
			total_count: 1,
			filters: vec![],
		}
	}

	#[rstest]
	fn list_view_renders_accessible_date_hierarchy_choices() {
		ReactiveScope::run(|| {
			// Arrange
			let data = list_data(vec![]);
			let date_hierarchy = DateHierarchyInfo {
				field: "published_at".to_string(),
				selection: DateHierarchySelection::default(),
				next_level: Some(DateHierarchyLevel::Year),
				choices: vec![2024, 2025],
			};
			let page_signal = Signal::new(1_u64);
			let filters_signal = Signal::new(HashMap::new());
			let query_params = Signal::new(DateHierarchyListQueryParams {
				list: ListQueryParams {
					page: Some(1),
					..ListQueryParams::default()
				},
				date_hierarchy: None,
			});
			let query_generation = std::rc::Rc::new(std::cell::Cell::new(0_u64));

			// Act
			let html = list_view_with_date_hierarchy(
				&data,
				page_signal,
				filters_signal,
				Some(&date_hierarchy),
				query_params,
				query_generation,
			)
			.render_to_string();

			// Assert
			let navigation = html
				.split_once("<nav ")
				.and_then(|(_, rest)| rest.split_once("</nav>"))
				.map(|(nav, _)| format!("<nav {nav}</nav>"))
				.expect("date hierarchy navigation should render");
			assert_eq!(
				navigation,
				concat!(
					"<nav class=\"admin-card p-4 mb-4\" aria-label=\"Date hierarchy\">",
					"<h2 class=\"text-xs font-semibold uppercase tracking-wider text-slate-500 mb-3\">Date hierarchy</h2>",
					"<p class=\"text-sm text-slate-600 mb-3\">published_at</p>",
					"<div class=\"flex flex-wrap gap-2 mb-3\"><button type=\"button\" class=\"admin-btn admin-btn-outline admin-btn-sm\" aria-label=\"Clear date hierarchy\">All</button></div>",
					"<div class=\"flex flex-wrap gap-2\"><button type=\"button\" class=\"admin-btn admin-btn-outline admin-btn-sm\" aria-label=\"Select year 2024\">2024</button>",
					"<button type=\"button\" class=\"admin-btn admin-btn-outline admin-btn-sm\" aria-label=\"Select year 2025\">2025</button></div>",
					"</nav>"
				)
			);
		});
	}

	#[rstest]
	fn date_hierarchy_choice_updates_the_requested_level_and_resets_page() {
		ReactiveScope::run(|| {
			let cases = [
				(
					DateHierarchyLevel::Year,
					2025,
					DateHierarchySelection {
						year: Some(2025),
						month: None,
						day: None,
					},
				),
				(
					DateHierarchyLevel::Month,
					7,
					DateHierarchySelection {
						year: Some(2020),
						month: Some(7),
						day: None,
					},
				),
				(
					DateHierarchyLevel::Day,
					9,
					DateHierarchySelection {
						year: Some(2020),
						month: Some(2),
						day: Some(9),
					},
				),
			];

			for (next_level, choice, expected_selection) in cases {
				// Arrange
				let query_params = Signal::new(DateHierarchyListQueryParams {
					list: ListQueryParams {
						page: Some(8),
						..ListQueryParams::default()
					},
					date_hierarchy: Some(DateHierarchySelection {
						year: Some(2020),
						month: Some(2),
						day: Some(3),
					}),
				});
				let query_generation = std::rc::Rc::new(std::cell::Cell::new(4_u64));

				// Act
				apply_date_hierarchy_choice(
					query_params,
					query_generation.clone(),
					DateHierarchySelection {
						year: Some(2020),
						month: Some(2),
						day: Some(3),
					},
					next_level,
					choice,
				);

				// Assert
				let params = query_params.get();
				assert_eq!(params.page, Some(1));
				assert_eq!(params.date_hierarchy, Some(expected_selection));
				assert_eq!(query_generation.get(), 5);
			}
		});
	}

	#[rstest]
	fn list_view_escapes_computed_column_text() {
		ReactiveScope::run(|| {
			// Arrange
			let mut record = HashMap::new();
			record.insert(
				"summary".to_string(),
				"</script><script>alert(1)</script>".to_string(),
			);
			let data = list_data(vec![record]);
			let page_signal = Signal::new(1_u64);
			let filters_signal = Signal::new(HashMap::new());

			// Act
			let html = list_view(&data, page_signal, filters_signal).render_to_string();

			// Assert
			assert!(html.contains("&lt;/script&gt;&lt;script&gt;alert(1)&lt;/script&gt;"));
			assert_eq!(html.matches("<script").count(), 0);
		});
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
	fn detail_view_links_to_object_history() {
		// Arrange
		let record = HashMap::from([("id".to_string(), "42".to_string())]);

		// Act
		let html = detail_view("User", "42", &record).render_to_string();

		// Assert
		assert!(html.contains("History"));
		assert!(html.contains("/admin/user/42/history/"));
	}

	#[rstest]
	fn edit_form_links_to_object_history() {
		// Act
		let html = model_form("User", &[], Some("42")).render_to_string();

		// Assert
		assert!(html.contains("History"));
		assert!(html.contains("/admin/user/42/history/"));
	}

	#[rstest]
	fn form_group_renders_presentation_and_stable_descriptions() {
		// Arrange
		let fields = vec![FormField {
			name: "title".to_string(),
			label: "Title".to_string(),
			spec: FormFieldSpec::Input {
				html_type: "text".to_string(),
			},
			required: true,
			nullable: false,
			value: String::new(),
		}];

		// Act
		let presentation = FieldPresentation {
			help_text: Some("Shown in the page title".to_string()),
			placeholder: Some("Write a headline".to_string()),
		};
		let html = form_group_with_presentation("Article", &fields[0], Some(&presentation))
			.render_to_string();

		// Assert
		assert!(html.contains(r#"id="field-title-help""#));
		assert!(html.contains("Shown in the page title"));
		assert!(html.contains(r#"placeholder="Write a headline""#));
		assert!(html.contains(r#"id="field-title-error""#));
		assert!(html.contains(r#"data-parent-field-error="true""#));
		assert!(html.contains(r#"aria-describedby="field-title-help field-title-error""#));
	}

	#[rstest]
	fn validation_errors_render_empty_parent_error_alert() {
		// Act
		let html = model_form("Article", &[], None).render_to_string();

		// Assert
		let start = html
			.find(r#"data-parent-form-error="true""#)
			.expect("parent form error node must render");
		let opening_start = html[..start]
			.rfind('<')
			.expect("error node has opening tag");
		let opening_end = html[start..]
			.find('>')
			.map(|offset| start + offset)
			.expect("error node opening tag closes");
		let opening = &html[opening_start..=opening_end];
		assert!(opening.contains(r#"role="alert""#));
		assert!(opening.contains(r#"aria-live="polite""#));
	}

	#[rstest]
	fn history_view_renders_privacy_safe_entries_and_list_navigation() {
		// Arrange
		let response = HistoryResponse {
			model_name: "User".to_string(),
			object_id: "42".to_string(),
			count: 1,
			page: 1,
			page_size: 25,
			total_pages: 1,
			results: vec![AdminHistoryEntry {
				id: 7,
				actor: "staff-7".to_string(),
				timestamp: "2026-08-09T01:02:03.000000Z".to_string(),
				action_name: "UPDATE".to_string(),
				model_name: "User".to_string(),
				object_id: "42".to_string(),
				object_repr: "User (42)".to_string(),
				changed_fields: vec!["email".to_string()],
				affected_count: 1,
				success: true,
			}],
		};

		// Act
		let html =
			ReactiveScope::run(|| history_view(&response, Signal::new(1)).render_to_string());

		// Assert
		assert!(html.contains("User History"));
		assert!(html.contains("staff-7"));
		assert!(html.contains("UPDATE"));
		assert!(html.contains("User (42)"));
		assert!(html.contains("email"));
		assert!(html.contains("/admin/user/"));
	}

	#[rstest]
	fn test_form_value_to_json_converts_id_values() {
		assert_eq!(
			form_value_to_json("owner_id", "42", false, false),
			json!(42)
		);
		assert_eq!(
			form_value_to_json("owner_id", "", false, false),
			serde_json::Value::Null
		);
		assert_eq!(form_value_to_json("title", "42", false, false), json!("42"));
	}

	#[rstest]
	fn relation_id_marker_preserves_text_primary_keys() {
		assert_eq!(
			form_value_to_json("owner_id", "001", false, true),
			json!("001")
		);
		assert_eq!(
			form_value_to_json("owner_id", "", false, true),
			serde_json::Value::Null
		);
		assert_eq!(
			form_value_to_json("owner_id", "42", false, false),
			json!(42)
		);
	}

	#[rstest]
	fn raw_relation_marks_textual_ids_and_describes_one_status_node() {
		let fields = vec![FormField {
			name: "author_id".to_string(),
			label: "Author".to_string(),
			spec: FormFieldSpec::Relation {
				field_name: "author".to_string(),
				widget: RelationWidget::RawId,
				selected: Some(RelationOption {
					id: "001".to_string(),
					label: "Ada Lovelace".to_string(),
				}),
				readonly: false,
			},
			required: true,
			nullable: false,
			value: "001".to_string(),
		}];

		let presentation = FieldPresentation {
			help_text: Some("Choose an author".to_string()),
			placeholder: Some("Enter an author ID".to_string()),
		};
		let html = form_group_with_presentation("Post", &fields[0], Some(&presentation))
			.render_to_string();

		assert_eq!(html.matches("data-relation-id=\"true\"").count(), 1);
		assert_eq!(html.matches("id=\"field-author_id-status\"").count(), 1);
		assert!(html.contains(
			r#"aria-describedby="field-author_id-help field-author_id-error field-author_id-status"#
		));
		assert!(html.contains(r#"id="field-author_id-error"#));
		assert!(html.contains(r#"data-parent-field-error="true"#));
		assert!(html.contains(r#"placeholder="Enter an author ID"#));
		assert!(html.contains("Ada Lovelace"));
	}

	#[rstest]
	fn required_autocomplete_marks_server_search_control_required() {
		// Arrange
		let fields = vec![FormField {
			name: "owner_id".to_string(),
			label: "Owner".to_string(),
			spec: FormFieldSpec::Relation {
				field_name: "owner".to_string(),
				widget: RelationWidget::Autocomplete,
				selected: None,
				readonly: false,
			},
			required: true,
			nullable: false,
			value: String::new(),
		}];

		// Act
		let presentation = FieldPresentation {
			help_text: Some("Choose an owner".to_string()),
			placeholder: Some("Search owners".to_string()),
		};
		let html = form_group_with_presentation("Post", &fields[0], Some(&presentation))
			.render_to_string();

		// Assert
		let search_start = html
			.find("id=\"field-owner_id-search\"")
			.expect("autocomplete search control must be rendered");
		let search_end = search_start
			+ html[search_start..]
				.find('>')
				.expect("autocomplete search control must be well-formed");
		assert!(html[search_start..search_end].contains("required"));
		assert!(html[search_start..search_end].contains(r#"placeholder="Search owners"#));
		assert!(html[search_start..search_end].contains(
			r#"aria-describedby="field-owner_id-help field-owner_id-error field-owner_id-status"#
		));
		assert!(html.contains(r#"id="field-owner_id-status" role="status"#));
	}

	#[rstest]
	fn many_to_many_selector_preserves_field_descriptions_on_visible_controls() {
		let field = FormField {
			name: "tags".to_string(),
			label: "Tags".to_string(),
			spec: FormFieldSpec::ManyToManySelector {
				layout: crate::types::RelationSelectorLayout::Horizontal,
				available: vec![RelationOption::new("1", "Rust")],
				selected: vec![RelationOption::new("2", "WebAssembly")],
				has_more: false,
			},
			required: false,
			nullable: false,
			value: String::new(),
		};

		let presentation = FieldPresentation {
			help_text: Some("Choose one or more tags".to_string()),
			placeholder: None,
		};
		let html = ReactiveScope::run(|| {
			form_group_with_presentation("Post", &field, Some(&presentation)).render_to_string()
		});

		let search_start = html
			.find("type=\"search\"")
			.expect("many-to-many search control must be rendered");
		let search_end = search_start
			+ html[search_start..]
				.find('>')
				.expect("many-to-many search control must be well-formed");
		let search_tag = &html[search_start..search_end];
		assert!(search_tag.contains("data-parent-validation-control=\"tags\""));
		assert!(search_tag.contains("field-tags-help field-tags-error"));
		assert!(search_tag.contains("-status\""));
		assert!(html.contains("data-parent-field-error=\"true\""));
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
	fn parsed_json_null_and_reserved_prefix_strings_keep_explicit_json_markers() {
		// Arrange
		let snapshots = [
			InlineControlSnapshot {
				object_id: Some("user-7".to_string()),
				field: Some("settings".to_string()),
				original: Some(json!({ "theme": "dark" })),
				current: tagged_inline_json_value(serde_json::Value::Null),
			},
			InlineControlSnapshot {
				object_id: Some("user-7".to_string()),
				field: Some("payload".to_string()),
				original: Some(json!("old")),
				current: tagged_inline_json_value(json!("__reinhardt_invalid_json__:value")),
			},
		];

		// Act
		let updates = inline_edit_updates(snapshots);

		// Assert
		assert_eq!(updates.len(), 1);
		assert_eq!(
			updates[0].json_fields,
			vec!["settings".to_string(), "payload".to_string()]
		);
		assert_eq!(
			updates[0].changes.get("settings"),
			Some(&serde_json::Value::Null)
		);
		assert_eq!(
			updates[0].changes.get("payload"),
			Some(&json!("__reinhardt_invalid_json__:value"))
		);
	}

	#[rstest]
	fn parsed_json_null_is_dirty_against_a_sql_null_original() {
		// Arrange
		let snapshots = [InlineControlSnapshot {
			object_id: Some("user-7".to_string()),
			field: Some("settings".to_string()),
			original: Some(serde_json::Value::Null),
			current: tagged_inline_json_value(serde_json::Value::Null),
		}];

		// Act
		let updates = inline_edit_updates(snapshots);

		// Assert
		assert_eq!(updates.len(), 1);
		assert_eq!(updates[0].json_fields, vec!["settings".to_string()]);
		assert_eq!(
			updates[0].changes.get("settings"),
			Some(&serde_json::Value::Null)
		);
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
		let nullable_boolean = FormFieldSpec::Select {
			choices: nullable_boolean_choices(),
		};

		// Act
		let text_kind = inline_value_kind(&text, &serde_json::Value::Null);
		let number_kind = inline_value_kind(&number, &serde_json::Value::Null);
		let checkbox_kind = inline_value_kind(&checkbox, &serde_json::Value::Null);
		let time_kind = inline_value_kind(&time, &json!("09:08:00"));
		let datetime_kind = inline_value_kind(&datetime, &json!("2026-08-10T09:08:07+09:00"));
		let nullable_boolean_kind = inline_value_kind(&nullable_boolean, &serde_json::Value::Null);
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
		assert_eq!(nullable_boolean_kind, InlineValueKind::NullableBoolean);
		assert_eq!(
			normalized_inline_original(&json!("09:08:00"), time_kind),
			json!("09:08")
		);
		assert_eq!(
			normalized_inline_original(&json!("09:08:07.123456"), time_kind),
			json!("09:08:07.123456")
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
		assert_eq!(
			inline_scalar_value(r#"{"id":42}"#, InlineValueKind::Json),
			json!({ "id": 42 })
		);
		assert_eq!(
			inline_scalar_value("", InlineValueKind::NullableBoolean),
			serde_json::Value::Null
		);
		assert_eq!(
			normalized_inline_original(&serde_json::Value::Null, nullable_boolean_kind),
			serde_json::Value::Null
		);
		assert_eq!(
			normalized_inline_original(&json!("read,write"), InlineValueKind::Array),
			json!(["read", "write"])
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
			nullable: false,
			value: normalized_inline_original(
				&json!("2026-08-10T09:08:07.123456Z"),
				InlineValueKind::DateTime,
			)
			.as_str()
			.expect("normalized date-time is a string")
			.to_string(),
		};

		let html = form_element_with_description_for_model(
			"",
			&field,
			"starts-at",
			"Starts at",
			"starts-at-error",
		)
		.render_to_string();

		assert!(html.contains(r#"type="datetime-local""#));
		assert!(html.contains(r#"step="any""#));
		assert!(html.contains(r#"value="2026-08-10T09:08:07.123456""#));
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
			nullable: true,
			value: String::new(),
		};

		// Act
		let html =
			form_element_with_description_for_model("", &field, "status", "Status", "status-error")
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
				nullable: false,
				step: None,
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
				nullable: false,
				step: None,
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
				nullable: false,
				step: None,
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
			data_table(&columns, &records, "User", "slug", None, Some(action)).render_to_string()
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
			nullable: false,
			step: None,
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
			data_table(&columns, &records, "User", "id", None, None).render_to_string()
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
			nullable: false,
			step: None,
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
			data_table(&columns, &records, "User", "uuid", None, None).render_to_string()
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
			nullable: false,
			step: None,
			form_spec: None,
		}];
		let records = vec![HashMap::from([
			("id".to_string(), json!(1)),
			("name".to_string(), json!("Alice")),
		])];

		// Act
		let html = ReactiveScope::run(|| {
			data_table(&columns, &records, "User", "id", None, None).render_to_string()
		});

		// Assert
		assert!(!html.contains(">Save<"));
		assert_eq!(html.matches("data-inline-editable").count(), 0);
	}
}

#[cfg(all(test, client))]
mod client_tests {
	use super::{FormField, model_form_with_configuration};
	use crate::types::{FormFieldSpec, PrepopulatedField};
	use reinhardt_pages::component::PageExt;
	use reinhardt_pages::dom::Element;
	use reinhardt_pages::reactive::ReactiveScope;
	use reinhardt_test::wasm::UserEvent;
	use wasm_bindgen::JsCast;
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	struct BodyRoot(web_sys::Element);

	impl BodyRoot {
		fn new(id: &str) -> Self {
			let document = web_sys::window()
				.expect("window")
				.document()
				.expect("document");
			let root = document.create_element("div").expect("root");
			root.set_id(id);
			document
				.body()
				.expect("body")
				.append_child(&root)
				.expect("append root");
			Self(root)
		}
	}

	impl Drop for BodyRoot {
		fn drop(&mut self) {
			reinhardt_pages::cleanup_reactive_nodes();
			self.0.remove();
		}
	}

	fn text_field(name: &str, value: &str) -> FormField {
		FormField {
			name: name.to_owned(),
			label: name.to_owned(),
			spec: FormFieldSpec::Input {
				html_type: "text".to_owned(),
			},
			required: false,
			value: value.to_owned(),
		}
	}

	fn input(root: &BodyRoot, name: &str) -> web_sys::HtmlInputElement {
		root.0
			.query_selector(&format!("input[name='{name}']"))
			.expect("query input")
			.expect("input exists")
			.dyn_into()
			.expect("text input")
	}

	#[wasm_bindgen_test]
	fn configured_form_prepopulation_updates_multisource_chain_without_dirtying_automatic_targets()
	{
		// Arrange
		let root = BodyRoot::new("admin-prepopulated-chain-test");
		let scope = ReactiveScope::new();
		let fields = vec![
			text_field("title", ""),
			text_field("category", ""),
			text_field("slug", ""),
			text_field("seo_slug", ""),
		];
		let rules = vec![
			PrepopulatedField::new("slug", ["title", "category"]),
			PrepopulatedField::new("seo_slug", ["slug"]),
		];
		let page = model_form_with_configuration("Article", &fields, &[], &[], None, &rules);
		scope.enter(|| {
			page.mount(&Element::new(root.0.clone()))
				.expect("mount configured form");
		});

		let title = input(&root, "title");
		let category = input(&root, "category");
		let slug = input(&root, "slug");
		let seo_slug = input(&root, "seo_slug");

		// Act: source edits update the multi-source target and its chain.
		UserEvent::type_text(&title, "Hello World");
		UserEvent::type_text(&category, "News");

		// Assert: automatic targets remain unlocked for later source edits.
		assert_eq!(slug.value(), "hello-world-news");
		assert_eq!(seo_slug.value(), "hello-world-news");
	}

	#[wasm_bindgen_test]
	fn configured_form_prepopulation_preserves_initial_and_manual_locks() {
		// Arrange
		let root = BodyRoot::new("admin-prepopulated-sticky-test");
		let scope = ReactiveScope::new();
		let fields = vec![
			text_field("title", "Initial Title"),
			text_field("slug", "existing-slug"),
			text_field("seo_slug", ""),
		];
		let rules = vec![
			PrepopulatedField::new("slug", ["title"]),
			PrepopulatedField::new("seo_slug", ["slug"]),
		];
		let page = model_form_with_configuration("Article", &fields, &[], &[], Some("1"), &rules);
		scope.enter(|| {
			page.mount(&Element::new(root.0.clone()))
				.expect("mount configured edit form");
		});

		let title = input(&root, "title");
		let slug = input(&root, "slug");
		let seo_slug = input(&root, "seo_slug");

		// Assert: a non-empty edit target remains locked while its downstream rule initializes.
		assert_eq!(slug.value(), "existing-slug");
		assert_eq!(seo_slug.value(), "existing-slug");

		// Act: source edits preserve the initial target and derived value.
		UserEvent::type_text(&title, "Changed Title");
		assert_eq!(slug.value(), "existing-slug");
		assert_eq!(seo_slug.value(), "existing-slug");

		// Act: manual edits and clearing remain sticky, while downstream rules follow.
		UserEvent::type_text(&slug, "manual-slug");
		assert_eq!(seo_slug.value(), "manual-slug");
		UserEvent::type_text(&title, "Later Title");
		assert_eq!(slug.value(), "manual-slug");
		assert_eq!(seo_slug.value(), "manual-slug");
		UserEvent::type_text(&slug, "");
		UserEvent::type_text(&title, "Final Title");
		assert_eq!(slug.value(), "");
		assert_eq!(seo_slug.value(), "");
	}

	#[wasm_bindgen_test]
	fn configured_form_prepopulation_is_sticky_and_chained() {
		let root = BodyRoot::new("admin-prepopulation-test");
		let scope = ReactiveScope::new();
		let fields = vec![text_field("title", ""), text_field("slug", "")];
		let rules = vec![PrepopulatedField::new("slug", ["title"])];
		let page = model_form_with_configuration("Article", &fields, &[], &[], None, &rules);
		scope.enter(|| {
			page.mount(&Element::new(root.0.clone()))
				.expect("mount form");
		});

		let title = input(&root, "title");
		let slug = input(&root, "slug");
		UserEvent::type_text(&title, "Hello World");
		assert_eq!(slug.value(), "hello-world");

		UserEvent::type_text(&slug, "manual");
		UserEvent::type_text(&title, "Changed Title");
		assert_eq!(slug.value(), "manual");

		UserEvent::type_text(&slug, "");
		UserEvent::type_text(&title, "Cleared Target");
		assert_eq!(slug.value(), "");
	}
}
