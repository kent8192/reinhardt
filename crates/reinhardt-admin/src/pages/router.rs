//! Client-side router for Reinhardt Admin Panel
//!
//! Handles routing between different admin pages:
//! - `/admin/login/` - Login form
//! - `/admin/` - Dashboard
//! - `/admin/{model}/` - List view
//! - `/admin/{model}/{id}/` - Detail view
//! - `/admin/{model}/add/` - Create form
//! - `/admin/{model}/{id}/change/` - Edit form
//! - `/admin/{model}/{id}/history/` - Per-object change history

// (Refs #4234) Migration to reinhardt_urls::routers::ClientRouter pending separate follow-up issue.
// `reinhardt_urls::routers::ClientRouter` is the canonical SPA router; this module
// references it pervasively (struct, `Router::new()`, `Arc<Router>`, closure params),
// so file-scope suppression is preferred over per-usage `#[allow(deprecated)]` attribute spam.
#[cfg(any(client, test))]
use crate::pages::components::features::json_value_to_display_string;
#[cfg(server)]
use crate::pages::components::features::list_view;
#[cfg(client)]
use crate::pages::components::features::list_view_with_actions;
#[cfg(client)]
use crate::pages::components::features::list_view_with_date_hierarchy;
use crate::pages::components::features::{
	Column, FormField, ListViewData, dashboard, decode_admin_path_segment, detail_view,
	history_view_with_route_model_name, model_form,
};
#[cfg(client)]
use crate::pages::components::features::{
	Column, FormField, ListViewData, dashboard, detail_view, list_view_with_actions_and_edit,
	model_form, model_form_with_fieldsets, model_form_with_inlines,
};
pub use crate::pages::components::login;
#[cfg(client)]
use crate::server::{
	execute_admin_action, get_dashboard, get_detail, get_fields, get_history,
	get_list_action_metadata, get_list_with_date_hierarchy, update_inline_edits,
};
#[cfg(client)]
use crate::types::DateHierarchyListResponse;
#[cfg(any(client, test))]
use crate::types::ListResponse;
#[cfg(client)]
use crate::types::{AdminActionRequest, DateHierarchyListQueryParams, InlineEditRequest};
use crate::types::{HistoryResponse, ModelInfo};
#[cfg(any(client, test))]
use reinhardt_pages::ResourceState;
use reinhardt_pages::Signal;
use reinhardt_pages::component::{Component, Page};
#[cfg(client)]
use reinhardt_pages::component::{MountError, PageExt};
use reinhardt_pages::page;
use reinhardt_pages::reactive::ReactiveScope;
use reinhardt_pages::router::Link;
#[cfg(client)]
use reinhardt_pages::use_resource;
use reinhardt_urls::routers::ClientRouter;
use reinhardt_urls::routers::client_router::Path;
#[cfg(any(client, test))]
use std::cell::Cell;
use std::cell::RefCell;
#[cfg(client)]
use std::collections::BTreeSet;
use std::collections::HashMap;
#[cfg(client)]
use std::rc::Rc;

/// Admin route enum
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum AdminRoute {
	/// Dashboard route
	Dashboard,
	/// List view route for a specific model.
	List {
		/// The name of the model to list.
		model_name: String,
	},
	/// Detail view route for a specific record.
	Detail {
		/// The name of the model.
		model_name: String,
		/// The record identifier.
		id: String,
	},
	/// Change-history route for a specific record.
	History {
		/// The name of the model.
		model_name: String,
		/// The record identifier.
		id: String,
	},
	/// Create form route for a specific model.
	Create {
		/// The name of the model to create.
		model_name: String,
	},
	/// Edit form route for a specific record.
	Edit {
		/// The name of the model.
		model_name: String,
		/// The record identifier to edit.
		id: String,
	},
	/// Not found route
	NotFound,
	/// Login route
	Login,
}

// Global Router instance
// Initialized by init_global_router() and accessed via with_router()
struct GlobalRouter {
	scope: ReactiveScope,
	router: ClientRouter,
	render_scope: RefCell<Option<ReactiveScope>>,
}

thread_local! {
	static ROUTER: RefCell<Option<GlobalRouter>> = const { RefCell::new(None) };
}

#[cfg(any(client, test))]
fn list_response_to_view_data(response: ListResponse) -> ListViewData {
	let pk_field = response.pk_field;
	ListViewData {
		model_name: response.model_name,
		columns: response
			.columns
			.map(|columns| {
				columns
					.into_iter()
					.map(|column| Column {
						field: column.field,
						label: column.label,
						sortable: column.sortable,
						editable: column.editable,
						linked: column.linked,
						required: column.required,
						nullable: column.nullable,
						step: column.step,
						form_spec: column.form_spec,
					})
					.collect()
			})
			.unwrap_or_else(|| {
				vec![Column {
					field: pk_field.clone(),
					label: pk_field.clone(),
					sortable: true,
					editable: false,
					linked: true,
					required: true,
					nullable: false,
					step: None,
					form_spec: None,
				}]
			}),
		pk_field,
		records: response.results,
		current_page: response.page,
		total_pages: response.total_pages,
		total_count: response.count,
		filters: response.available_filters.unwrap_or_default(),
	}
}

#[cfg(any(client, test))]
fn begin_list_request(latest_generation: &Cell<u64>) -> u64 {
	let generation = latest_generation.get().wrapping_add(1);
	latest_generation.set(generation);
	generation
}

#[cfg(client)]
fn invalidate_list_request(latest_generation: &Cell<u64>) {
	latest_generation.set(latest_generation.get().wrapping_add(1));
}

#[cfg(any(client, test))]
fn commit_list_request(
	latest_generation: &Cell<u64>,
	generation: u64,
	result: Result<ListResponse, String>,
	rendered_state: Signal<ResourceState<ListResponse, String>>,
	page_signal: Signal<u64>,
) {
	if generation != latest_generation.get() {
		return;
	}

	match result {
		Ok(response) => {
			if page_signal.get_untracked() != response.page {
				page_signal.set(response.page);
			}
			rendered_state.set(ResourceState::Success(response));
		}
		Err(error) => rendered_state.set(ResourceState::Error(error)),
	}
}

#[cfg(client)]
fn commit_date_hierarchy_list_request(
	latest_generation: &Cell<u64>,
	generation: u64,
	result: Result<DateHierarchyListResponse, String>,
	rendered_state: Signal<ResourceState<DateHierarchyListResponse, String>>,
	page_signal: Signal<u64>,
) {
	if generation != latest_generation.get() {
		return;
	}

	match result {
		Ok(response) => {
			if page_signal.get_untracked() != response.response.page {
				page_signal.set(response.response.page);
			}
			rendered_state.set(ResourceState::Success(response));
		}
		Err(error) => rendered_state.set(ResourceState::Error(error)),
	}
}

/// Admin URL configuration loaded from server at runtime.
///
/// Stored in a thread-local (safe because WASM is single-threaded) and
/// populated when the dashboard response is received. Falls back to
/// defaults if not yet initialized.
#[cfg(client)]
#[derive(Clone)]
struct AdminUrls {
	login_url: String,
	logout_url: String,
}

#[cfg(client)]
impl Default for AdminUrls {
	fn default() -> Self {
		Self {
			login_url: "/admin/login/".to_string(),
			logout_url: "/admin/logout/".to_string(),
		}
	}
}

#[cfg(client)]
thread_local! {
	static ADMIN_URLS: RefCell<AdminUrls> = RefCell::new(AdminUrls::default());
}

/// Returns the configured login URL, with a trailing slash.
#[cfg(client)]
pub(crate) fn get_login_url() -> String {
	ADMIN_URLS.with(|u| u.borrow().login_url.clone())
}

/// Initialize the global router instance
///
/// This must be called once at application startup before any routing operations.
///
/// # Example
///
/// ```no_run
/// use reinhardt_admin::pages::router::init_global_router;
///
/// init_global_router();
/// ```
pub fn init_global_router() {
	let scope = ReactiveScope::new();
	let router = scope.enter(init_router);
	let previous = ROUTER.with(|stored| {
		stored.borrow_mut().replace(GlobalRouter {
			scope,
			router,
			render_scope: RefCell::new(None),
		})
	});
	drop(previous);
}

/// Provides access to the global router instance
///
/// Returns `None` if the router has not been initialized via `init_global_router()`.
///
/// # Example
///
/// ```no_run
/// use reinhardt_admin::pages::router::try_with_router;
///
/// if let Some(count) = try_with_router(|router| router.route_count()) {
///     println!("Routes: {}", count);
/// }
/// ```
pub fn try_with_router<F, R>(f: F) -> Option<R>
where
	F: FnOnce(&ClientRouter) -> R,
{
	ROUTER.with(|stored| {
		let stored = stored.borrow();
		stored
			.as_ref()
			.map(|stored| stored.scope.enter(|| f(&stored.router)))
	})
}

/// Provides access to the global router instance
///
/// # Panics
///
/// Panics if the router has not been initialized via `init_global_router()`.
/// Prefer `try_with_router` for non-panicking access.
///
/// # Example
///
/// ```no_run
/// use reinhardt_admin::pages::router::with_router;
///
/// with_router(|router| {
///     let params = router.current_params().get();
///     // Use router...
/// });
/// ```
pub fn with_router<F, R>(f: F) -> R
where
	F: FnOnce(&ClientRouter) -> R,
{
	try_with_router(f).expect("Router not initialized. Call init_global_router() first.")
}

/// Renders the current route in a scope owned by the mounted admin page.
///
/// Router navigation signals remain in the router scope, while route-local
/// signals, resources, and page arena nodes are disposed before the next mount.
pub fn render_current_route() -> Page {
	ROUTER.with(|stored| {
		let stored = stored.borrow();
		let stored = stored
			.as_ref()
			.expect("Router not initialized. Call init_global_router() first.");
		let render_scope = ReactiveScope::new();
		let page = render_scope.enter(|| stored.router.render_current());
		let previous = stored.render_scope.borrow_mut().replace(render_scope);
		drop(previous);
		page
	})
}

/// Renders and mounts the current route while its route scope is active.
///
/// Reactive pages create effects during mounting, so building the page under
/// the route scope alone is insufficient for client-side mounts.
#[cfg(client)]
pub fn mount_current_route(parent: &Element) -> Result<(), MountError> {
	ROUTER.with(|stored| {
		let stored = stored.borrow();
		let stored = stored
			.as_ref()
			.expect("Router not initialized. Call init_global_router() first.");
		let render_scope = ReactiveScope::new();
		let result = render_scope.enter(|| stored.router.render_current().mount(parent));
		if result.is_ok() {
			let previous = stored.render_scope.borrow_mut().replace(render_scope);
			drop(previous);
		}
		result
	})
}

/// Dashboard view component for router
#[cfg(client)]
fn dashboard_view() -> Page {
	let dashboard_resource = use_resource(
		|| async { get_dashboard().await.map_err(|e| e.to_string()) },
		deps![],
	);

	let reactive_content = Page::reactive({
		let resource = dashboard_resource.clone();
		move || match resource.get() {
			ResourceState::Loading => loading_view(),
			ResourceState::Success(data) => {
				// Store login/logout URLs from server settings
				ADMIN_URLS.with(|urls| {
					let mut urls = urls.borrow_mut();
					urls.login_url = format!("{}/", data.login_url.trim_end_matches('/'));
					urls.logout_url = format!("{}/", data.logout_url.trim_end_matches('/'));
				});
				dashboard(&data.site_header, &data.models)
			}
			ResourceState::Error(err) => error_view(&err),
		}
	});

	page!(|reactive_content: Page| {
		div {
			class: "dashboard-container p-6 md:p-8 max-w-7xl mx-auto",
			{ reactive_content }
		}
	})(reactive_content)
}

/// Dashboard view component for router (non-WASM fallback)
#[cfg(server)]
fn dashboard_view() -> Page {
	// Dummy data for non-WASM environments (tests, etc.)
	let models = vec![
		ModelInfo {
			name: "Users".to_string(),
			list_url: "/admin/users/".to_string(),
		},
		ModelInfo {
			name: "Posts".to_string(),
			list_url: "/admin/posts/".to_string(),
		},
	];

	dashboard("Administration", &models)
}

/// List view component for router
#[cfg(client)]
fn list_view_component(model_name: String) -> Page {
	use reinhardt_pages::use_action;
	use reinhardt_pages::use_retained_effect;

	let list_model_name = model_name.clone();
	let model_name_for_save = model_name.clone();
	let query_params = Signal::new(DateHierarchyListQueryParams::default());
	let query_generation = Rc::new(Cell::new(0_u64));
	let list_resource = use_resource(
		move || {
			let model_name = list_model_name.clone();
			let params = query_params.get();
			async move {
				let response = get_list_with_date_hierarchy(model_name.clone(), params)
					.await
					.map_err(|e| e.to_string())?;
				let metadata = get_list_action_metadata(model_name)
					.await
					.map_err(|e| e.to_string())?;
				Ok::<_, String>((response, metadata))
			}
		},
		deps![query_params],
	);
	let save_action = use_action(move |request: InlineEditRequest| {
		let model_name = model_name_for_save.clone();
		async move {
			update_inline_edits(model_name, request)
				.await
				.map_err(|_| "Save failed".to_string())
		}
	})
	.on_success({
		let resource = list_resource.clone();
		move |response| {
			if response.errors.is_empty() {
				resource.refetch();
			} else {
				crate::pages::components::features::set_inline_edit_controls_disabled(false);
			}
		}
	})
	.on_error(|_| {
		crate::pages::components::features::set_inline_edit_controls_disabled(false);
	});

	// Create signals outside the reactive closure so they persist across re-renders
	let page_signal = Signal::new(1u64);
	let filters_signal = Signal::new(HashMap::new());
	let selected_ids = Signal::new(BTreeSet::new());
	let selected_action = Signal::new(String::new());
	let action_model_name = model_name;
	let list_resource_for_success = list_resource.clone();
	let action = use_action(move |request: AdminActionRequest| {
		let model_name = action_model_name.clone();
		async move {
			execute_admin_action(model_name, request)
				.await
				.map_err(|error| error.to_string())
		}
	})
	.on_success(move |_| {
		selected_ids.set(BTreeSet::new());
		list_resource_for_success.refetch();
	});

	use_retained_effect(
		move || {
			let page = page_signal.get_untracked();
			let mut params = query_params.get_untracked();
			if params.page != Some(page) {
				params.page = Some(page);
				query_params.set(params);
			}
			None::<fn()>
		},
		deps![page_signal],
	);

	// Sync page_signal from the completed resource outside the rendering closure.
	{
		let resource = list_resource.clone();
		let resource_for_deps = list_resource.clone();
		use_retained_effect(
			move || {
				if let ResourceState::Success((ref response, _)) = resource.get() {
					page_signal.set(response.response.page);
					selected_ids.set(BTreeSet::new());
				}
				None::<fn()>
			},
			deps![resource_for_deps],
		);
	}

	let reactive_content = Page::reactive({
		let resource = list_resource.clone();
		move || match resource.get() {
			ResourceState::Loading => loading_view(),
			ResourceState::Success((response, metadata)) => {
				let data = list_response_to_view_data(response.response);
				list_view_with_actions_and_edit(
					&data,
					&metadata.pk_field,
					&metadata.actions,
					page_signal,
					filters_signal,
					response.date_hierarchy.as_ref(),
					query_params,
					query_generation.clone(),
					(selected_ids, selected_action, action),
					save_action,
				)
			}
			ResourceState::Error(err) => error_view(&err),
		}
	});

	page!(|reactive_content: Page| {
		div {
			class: "list-container p-6 md:p-8 max-w-7xl mx-auto",
			{ reactive_content }
		}
	})(reactive_content)
}

/// List view component for router (non-WASM fallback)
#[cfg(server)]
fn list_view_component(model_name: String) -> Page {
	use std::collections::HashMap;

	// Dummy data for non-WASM environments (tests, etc.)
	let data = ListViewData {
		model_name: model_name.clone(),
		columns: vec![
			Column {
				field: "id".to_string(),
				label: "ID".to_string(),
				sortable: true,
				editable: false,
				linked: true,
				required: true,
				nullable: false,
				step: None,
				form_spec: None,
			},
			Column {
				field: "name".to_string(),
				label: "Name".to_string(),
				sortable: true,
				editable: false,
				linked: false,
				required: false,
				nullable: false,
				step: None,
				form_spec: None,
			},
		],
		pk_field: "id".to_string(),
		records: vec![],
		current_page: 1,
		total_pages: 1,
		total_count: 0,
		filters: vec![],
	};

	let page_signal = Signal::new(1_u64);
	let filters_signal = Signal::new(HashMap::new());
	list_view(&data, page_signal, filters_signal)
}

/// Detail view component for router
#[cfg(client)]
fn detail_view_component(model_name: String, record_id: String) -> Page {
	let model_name_for_view = model_name.clone();
	let record_id_for_view = record_id.clone();
	let detail_resource = use_resource(
		move || {
			let model_name = model_name.clone();
			let record_id = record_id.clone();
			async move {
				get_detail(model_name, record_id)
					.await
					.map_err(|e| e.to_string())
			}
		},
		deps![],
	);

	let reactive_content = Page::reactive({
		let resource = detail_resource.clone();
		let model_name = model_name_for_view;
		let record_id = record_id_for_view;
		move || match resource.get() {
			ResourceState::Loading => loading_view(),
			ResourceState::Success(response) => {
				let data: std::collections::HashMap<String, String> = response
					.data
					.iter()
					.map(|(k, v)| (k.clone(), json_value_to_display_string(v)))
					.collect();
				detail_view(&model_name, &record_id, &data)
			}
			ResourceState::Error(err) => error_view(&err),
		}
	});

	page!(|reactive_content: Page| {
		div {
			class: "detail-container p-6 md:p-8 max-w-7xl mx-auto",
			{ reactive_content }
		}
	})(reactive_content)
}

/// Detail view component for router (non-WASM fallback)
#[cfg(server)]
fn detail_view_component(model_name: String, record_id: String) -> Page {
	// Dummy data for non-WASM environments (tests, etc.)
	let mut record = HashMap::new();
	record.insert("id".to_string(), record_id.clone());
	record.insert("name".to_string(), "Sample Record".to_string());

	detail_view(&model_name, &record_id, &record)
}

/// Object history view component for router
#[cfg(client)]
fn history_view_component(model_name: String, record_id: String) -> Page {
	let page_signal = Signal::new(1_u64);
	let route_model_name = model_name.clone();
	let history_resource = use_resource(
		move || {
			let model_name = model_name.clone();
			let record_id = record_id.clone();
			let page = page_signal.get();
			async move {
				get_history(model_name, record_id, page)
					.await
					.map_err(|error| error.to_string())
			}
		},
		deps![page_signal],
	);

	let reactive_content = Page::reactive({
		let resource = history_resource.clone();
		move || match resource.get() {
			ResourceState::Loading => loading_view(),
			ResourceState::Success(response) => {
				history_view_with_route_model_name(&response, page_signal, &route_model_name)
			}
			ResourceState::Error(error) => error_view(&error),
		}
	});

	page!(|reactive_content: Page| {
		div {
			class: "history-container p-6 md:p-8 max-w-7xl mx-auto",
			{ reactive_content }
		}
	})(reactive_content)
}

/// Object history view component for router (non-WASM fallback)
#[cfg(server)]
fn history_view_component(model_name: String, record_id: String) -> Page {
	let response = HistoryResponse {
		model_name: model_name.clone(),
		object_id: record_id,
		count: 0,
		page: 1,
		page_size: 25,
		total_pages: 1,
		results: Vec::new(),
	};
	history_view_with_route_model_name(&response, Signal::new(1), &model_name)
}

/// Create form view component for router
#[cfg(client)]
fn create_view_component(model_name: String) -> Page {
	let model_name_for_view = model_name.clone();
	let fields_resource = use_resource(
		move || {
			let model_name = model_name.clone();
			async move {
				get_fields(model_name, None)
					.await
					.map_err(|e| e.to_string())
			}
		},
		deps![],
	);

	let reactive_content = Page::reactive({
		let resource = fields_resource.clone();
		let model_name = model_name_for_view;
		move || match resource.get() {
			ResourceState::Loading => loading_view(),
			ResourceState::Success(response) => {
				let fields: Vec<FormField> = response
					.fields
					.into_iter()
					.map(|field_info| FormField {
						spec: crate::types::FormFieldSpec::from(&field_info.field_type),
						name: field_info.name,
						label: field_info.label,
						required: field_info.required,
						nullable: field_info.nullable,
						value: String::new(),
					})
					.collect();
				if response.inlines.is_empty() {
					if let Some(fieldsets) = response.fieldsets {
						model_form_with_fieldsets(&model_name, &fields, &fieldsets, None)
					} else {
						model_form(&model_name, &fields, None)
					}
				} else {
					let fieldsets = response.fieldsets.unwrap_or_default();
					model_form_with_inlines(
						&model_name,
						&fields,
						&fieldsets,
						&response.inlines,
						None,
					)
				}
			}
			ResourceState::Error(err) => error_view(&err),
		}
	});

	page!(|reactive_content: Page| {
		div {
			class: "form-container p-6 md:p-8 max-w-7xl mx-auto",
			{ reactive_content }
		}
	})(reactive_content)
}

/// Create form view component for router (non-WASM fallback)
#[cfg(server)]
fn create_view_component(model_name: String) -> Page {
	// Dummy data for non-WASM environments (tests, etc.)
	let fields = vec![
		FormField {
			name: "name".to_string(),
			label: "Name".to_string(),
			spec: crate::types::FormFieldSpec::Input {
				html_type: "text".to_string(),
			},
			required: true,
			nullable: false,
			value: String::new(),
		},
		FormField {
			name: "email".to_string(),
			label: "Email".to_string(),
			spec: crate::types::FormFieldSpec::Input {
				html_type: "email".to_string(),
			},
			required: true,
			nullable: false,
			value: String::new(),
		},
	];

	model_form(&model_name, &fields, None)
}

/// Edit form view component for router
#[cfg(client)]
fn edit_view_component(model_name: String, record_id: String) -> Page {
	let model_name_for_view = model_name.clone();
	let record_id_for_view = record_id.clone();
	let fields_resource = use_resource(
		move || {
			let model_name = model_name.clone();
			let record_id = record_id.clone();
			async move {
				get_fields(model_name, Some(record_id))
					.await
					.map_err(|e| e.to_string())
			}
		},
		deps![],
	);

	let reactive_content = Page::reactive({
		let resource = fields_resource.clone();
		let model_name = model_name_for_view;
		let record_id = record_id_for_view;
		move || match resource.get() {
			ResourceState::Loading => loading_view(),
			ResourceState::Success(response) => {
				let fields: Vec<FormField> = response
					.fields
					.into_iter()
					.map(|field_info| {
						let value = if let Some(ref vals) = response.values {
							match vals.get(&field_info.name) {
								// Multi-valued arrays are flattened to a comma-separated
								// string until FormField regains first-class multi-value support.
								Some(v) if v.is_array() => v
									.as_array()
									.map(|arr| {
										arr.iter()
											.map(json_value_to_display_string)
											.collect::<Vec<_>>()
											.join(", ")
									})
									.unwrap_or_default(),
								Some(v) => json_value_to_display_string(v),
								None => String::new(),
							}
						} else {
							String::new()
						};

						FormField {
							spec: crate::types::FormFieldSpec::from(&field_info.field_type),
							name: field_info.name,
							label: field_info.label,
							required: field_info.required,
							nullable: field_info.nullable,
							value,
						}
					})
					.collect();
				if response.inlines.is_empty() {
					if let Some(fieldsets) = response.fieldsets {
						model_form_with_fieldsets(
							&model_name,
							&fields,
							&fieldsets,
							Some(&record_id),
						)
					} else {
						model_form(&model_name, &fields, Some(&record_id))
					}
				} else {
					let fieldsets = response.fieldsets.unwrap_or_default();
					model_form_with_inlines(
						&model_name,
						&fields,
						&fieldsets,
						&response.inlines,
						Some(&record_id),
					)
				}
			}
			ResourceState::Error(err) => error_view(&err),
		}
	});

	page!(|reactive_content: Page| {
		div {
			class: "form-container p-6 md:p-8 max-w-7xl mx-auto",
			{ reactive_content }
		}
	})(reactive_content)
}

/// Edit form view component for router (non-WASM fallback)
#[cfg(server)]
fn edit_view_component(model_name: String, record_id: String) -> Page {
	// Dummy data for non-WASM environments (tests, etc.)
	let fields = vec![
		FormField {
			name: "name".to_string(),
			label: "Name".to_string(),
			spec: crate::types::FormFieldSpec::Input {
				html_type: "text".to_string(),
			},
			required: true,
			nullable: false,
			value: "Existing Value".to_string(),
		},
		FormField {
			name: "email".to_string(),
			label: "Email".to_string(),
			spec: crate::types::FormFieldSpec::Input {
				html_type: "email".to_string(),
			},
			required: true,
			nullable: false,
			value: "user@example.com".to_string(),
		},
	];

	model_form(&model_name, &fields, Some(&record_id))
}

/// Not found view component for router
fn not_found_view() -> Page {
	let dashboard_link = Link::new("/admin/", "Go to Dashboard")
		.class("admin-btn admin-btn-primary")
		.render();

	page!(|dashboard_link: Page| {
		div {
			class: "not-found text-center py-16 animate__animated animate__fadeIn",
			h1 {
				class: "font-display text-4xl font-bold text-slate-300 mb-2",
				"404"
			}
			p {
				class: "text-slate-500 mb-6",
				"The requested page could not be found."
			}
			div { { dashboard_link } }
		}
	})(dashboard_link)
}

/// Loading view component
///
/// Displays a loading indicator while data is being fetched.
#[cfg(client)]
fn loading_view() -> Page {
	page!(|| {
		div {
			class: "flex justify-center items-center py-16",
			div {
				class: "admin-spinner",
				role: "status",
				span {
					class: "sr-only",
					"Loading..."
				}
			}
		}
	})()
}

/// Error view component
///
/// Displays an error message when data fetch fails.
/// If the error indicates a 401 Unauthorized response, clears the JWT
/// token and redirects to the login page.
#[cfg(client)]
fn error_view(message: &str) -> Page {
	// Detect 401 Unauthorized — clear token and redirect to login
	if message.contains("401") {
		reinhardt_pages::auth::clear_jwt_token();
		reinhardt_pages::auth::auth_state().logout();
		let login_url = get_login_url();
		with_router(|r| {
			let _ = r.push(&login_url);
		});
		return page!(|| {
			div {
				class: "text-center py-12 text-slate-500",
				"Redirecting to login..."
			}
		})();
	}

	let message = message.to_string();
	let dashboard_link = Link::new("/admin/", "Go to Dashboard")
		.class("admin-btn admin-btn-primary")
		.render();

	page!(|message: String, dashboard_link: Page| {
		div {
			class: "admin-alert admin-alert-danger mt-8 animate__animated animate__shakeX",
			role: "alert",
			h4 {
				class: "font-semibold mb-2",
				"Error"
			}
			p {
				class: "mb-4",
				{ message }
			}
			{ dashboard_link }
		}
	})(message, dashboard_link)
}

/// Initialize the admin router
///
/// # Route registration order
///
/// Routes are registered in a specific order to ensure correct matching.
/// More specific routes (with literal path segments) must be registered
/// before less specific routes (with only dynamic parameters):
///
/// 1. `/admin/` - dashboard (exact match)
/// 2. `/admin/{model}/add/` - create (literal `add` segment)
/// 3. `/admin/{model}/{id}/change/` - edit (literal `change` segment)
/// 4. `/admin/{model}/{id}/history/` - history (literal `history` segment)
/// 5. `/admin/{model}/{id}/` - detail (all dynamic segments)
/// 6. `/admin/{model}/` - list (all dynamic segments)
///
/// If `detail` were registered before `create`, a request to
/// `/admin/users/add/` would incorrectly match the detail route
/// with `id="add"`.
///
/// # Example
///
/// ```no_run
/// use reinhardt_admin::pages::router::init_router;
///
/// let router = init_router();
/// ```
pub fn init_router() -> ClientRouter {
	// IMPORTANT: Route registration order matters. See doc comment above.
	// Login route must be registered before dynamic routes to prevent
	// /admin/login/ from matching the list route with model="login".
	ClientRouter::new()
		.route("login", "/admin/login/", login::login_view)
		.route("dashboard", "/admin/", dashboard_view)
		.route_path(
			"create",
			"/admin/{model}/add/",
			|Path(model_name): Path<String>| create_view_component(model_name),
		)
		.route_path(
			"edit",
			"/admin/{model}/{id}/change/",
			|Path(model_name): Path<String>, Path(record_id): Path<String>| {
				edit_view_component(model_name, decode_admin_path_segment(&record_id))
			},
		)
		.route_path(
			"history",
			"/admin/{model}/{id}/history/",
			|Path(model_name): Path<String>, Path(record_id): Path<String>| {
				history_view_component(model_name, decode_admin_path_segment(&record_id))
			},
		)
		.route_path(
			"detail",
			"/admin/{model}/{id}/",
			|Path(model_name): Path<String>, Path(record_id): Path<String>| {
				detail_view_component(model_name, decode_admin_path_segment(&record_id))
			},
		)
		.route_path(
			"list",
			"/admin/{model}/",
			|Path(model_name): Path<String>| list_view_component(model_name),
		)
		.not_found(not_found_view)
}

#[cfg(all(test, server))]
mod tests {
	use super::*;
	use reinhardt_core::reactive::ReactiveScope;
	use rstest::{fixture, rstest};
	use serial_test::serial;

	fn clear_global_router() {
		let previous = ROUTER.with(|router| router.borrow_mut().take());
		drop(previous);
	}

	struct InitializedGlobalRouter;

	impl Drop for InitializedGlobalRouter {
		fn drop(&mut self) {
			clear_global_router();
		}
	}

	#[fixture]
	fn initialized_global_router() -> InitializedGlobalRouter {
		clear_global_router();
		ReactiveScope::run(init_global_router);
		InitializedGlobalRouter
	}

	#[test]
	fn test_admin_route_enum() {
		let route = AdminRoute::Dashboard;
		assert_eq!(route, AdminRoute::Dashboard);

		let route = AdminRoute::List {
			model_name: "users".to_string(),
		};
		assert!(matches!(route, AdminRoute::List { .. }));
	}

	#[test]
	fn test_init_router_creates_routes() {
		ReactiveScope::run(|| {
			let router = init_router();
			assert_eq!(router.route_count(), 7);
			assert!(router.has_route("login"));
			assert!(router.has_route("dashboard"));
			assert!(router.has_route("list"));
			assert!(router.has_route("detail"));
			assert!(router.has_route("create"));
			assert!(router.has_route("edit"));
			assert!(router.has_route("history"));
		});
	}

	#[test]
	fn test_dashboard_route_match() {
		ReactiveScope::run(|| {
			let router = init_router();
			let route_match = router.match_path("/admin/");
			assert!(route_match.is_some());

			let route_match = route_match.unwrap();
			assert_eq!(route_match.route.name(), Some("dashboard"));
		});
	}

	#[test]
	fn test_list_route_match() {
		ReactiveScope::run(|| {
			let router = init_router();
			let route_match = router.match_path("/admin/users/");
			assert!(route_match.is_some());

			let route_match = route_match.unwrap();
			assert_eq!(route_match.route.name(), Some("list"));
			assert_eq!(
				route_match.params.get("model").map(String::as_str),
				Some("users")
			);
		});
	}

	#[test]
	fn test_detail_route_match() {
		ReactiveScope::run(|| {
			let router = init_router();
			let route_match = router.match_path("/admin/users/42/");
			assert!(route_match.is_some());

			let route_match = route_match.unwrap();
			assert_eq!(route_match.route.name(), Some("detail"));
			assert_eq!(
				route_match.params.get("model").map(String::as_str),
				Some("users")
			);
			assert_eq!(route_match.params.get("id").map(String::as_str), Some("42"));
		});
	}

	#[rstest]
	fn history_route_matches_before_detail_and_reverses() {
		ReactiveScope::run(|| {
			// Arrange
			let router = init_router();

			// Act
			let route_match = router
				.match_path("/admin/users/42/history/")
				.expect("history route must match");
			let reversed = router
				.reverse("history", &[("model", "users"), ("id", "42")])
				.expect("history route must reverse");

			// Assert
			assert_eq!(route_match.route.name(), Some("history"));
			assert_eq!(
				route_match.params.get("model").map(String::as_str),
				Some("users")
			);
			assert_eq!(route_match.params.get("id").map(String::as_str), Some("42"));
			assert_eq!(reversed, "/admin/users/42/history/");
		});
	}

	#[test]
	fn test_create_route_match() {
		ReactiveScope::run(|| {
			let router = init_router();
			let route_match = router.match_path("/admin/users/add/");
			assert!(route_match.is_some());

			let route_match = route_match.unwrap();
			assert_eq!(route_match.route.name(), Some("create"));
			assert_eq!(
				route_match.params.get("model").map(String::as_str),
				Some("users")
			);
		});
	}

	#[test]
	fn test_edit_route_match() {
		ReactiveScope::run(|| {
			let router = init_router();
			let route_match = router.match_path("/admin/users/42/change/");
			assert!(route_match.is_some());

			let route_match = route_match.unwrap();
			assert_eq!(route_match.route.name(), Some("edit"));
			assert_eq!(
				route_match.params.get("model").map(String::as_str),
				Some("users")
			);
			assert_eq!(route_match.params.get("id").map(String::as_str), Some("42"));
		});
	}

	#[test]
	fn test_reverse_url_dashboard() {
		ReactiveScope::run(|| {
			let router = init_router();
			let url = router.reverse("dashboard", &[]).unwrap();
			assert_eq!(url, "/admin/");
		});
	}

	#[test]
	fn test_reverse_url_list() {
		ReactiveScope::run(|| {
			let router = init_router();
			let url = router.reverse("list", &[("model", "users")]).unwrap();
			assert_eq!(url, "/admin/users/");
		});
	}

	#[test]
	fn test_reverse_url_detail() {
		ReactiveScope::run(|| {
			let router = init_router();
			let url = router
				.reverse("detail", &[("model", "users"), ("id", "42")])
				.unwrap();
			assert_eq!(url, "/admin/users/42/");
		});
	}

	#[test]
	fn test_reverse_url_create() {
		ReactiveScope::run(|| {
			let router = init_router();
			let url = router.reverse("create", &[("model", "users")]).unwrap();
			assert_eq!(url, "/admin/users/add/");
		});
	}

	#[test]
	fn test_reverse_url_edit() {
		ReactiveScope::run(|| {
			let router = init_router();
			let url = router
				.reverse("edit", &[("model", "users"), ("id", "42")])
				.unwrap();
			assert_eq!(url, "/admin/users/42/change/");
		});
	}

	#[rstest]
	#[serial(global_router)]
	fn test_init_global_router(_initialized_global_router: InitializedGlobalRouter) {
		with_router(|router| {
			assert_eq!(router.route_count(), 7);
			assert!(router.has_route("login"));
			assert!(router.has_route("dashboard"));
			assert!(router.has_route("list"));
			assert!(router.has_route("detail"));
			assert!(router.has_route("create"));
			assert!(router.has_route("edit"));
			assert!(router.has_route("history"));
		});
	}

	#[rstest]
	#[serial(global_router)]
	fn global_router_scope_outlives_initializer_scope(
		_initialized_global_router: InitializedGlobalRouter,
	) {
		with_router(|router| {
			assert_eq!(router.current_path().get(), "/");
		});
	}

	#[rstest]
	#[serial(global_router)]
	fn test_with_router_access(_initialized_global_router: InitializedGlobalRouter) {
		let route_count = with_router(|router| router.route_count());
		assert_eq!(route_count, 7);

		let has_dashboard = with_router(|router| router.has_route("dashboard"));
		assert!(has_dashboard);
	}

	#[test]
	#[should_panic(expected = "Router not initialized")]
	fn test_with_router_panics_when_not_initialized() {
		clear_global_router();

		with_router(|_| {});
	}

	#[test]
	fn test_try_with_router_returns_none_when_not_initialized() {
		clear_global_router();

		let result = try_with_router(|router| router.route_count());
		assert!(result.is_none());
	}

	#[test]
	fn test_try_with_router_returns_some_when_initialized() {
		ReactiveScope::run(|| {
			init_global_router();

			let result = try_with_router(|router| router.route_count());
			assert_eq!(result, Some(7));
		});
	}

	#[test]
	fn test_list_view_with_model_name() {
		let html = ReactiveScope::run(|| {
			let view = list_view_component("users".to_string());
			view.render_to_string()
		});
		assert!(html.contains("users") || html.contains("List"));
	}

	#[rstest]
	fn newer_list_response_wins_when_requests_complete_in_reverse_order() {
		ReactiveScope::run(|| {
			// Arrange
			let latest_generation = std::cell::Cell::new(0_u64);
			let older_generation = begin_list_request(&latest_generation);
			let newer_generation = begin_list_request(&latest_generation);
			let rendered_state = Signal::new(ResourceState::Loading);
			let page_signal = Signal::new(8_u64);
			let response = |model_name: &str, page| crate::types::ListResponse {
				model_name: model_name.to_string(),
				pk_field: "id".to_string(),
				count: 1,
				page,
				page_size: 1,
				total_pages: 8,
				results: vec![],
				available_filters: None,
				columns: None,
			};

			// Act
			commit_list_request(
				&latest_generation,
				newer_generation,
				Ok(response("newer", 1)),
				rendered_state,
				page_signal,
			);
			commit_list_request(
				&latest_generation,
				older_generation,
				Ok(response("older", 8)),
				rendered_state,
				page_signal,
			);

			// Assert
			assert_eq!(page_signal.get(), 1);
			match rendered_state.get() {
				ResourceState::Success(response) => {
					assert_eq!(response.model_name, "newer");
					assert_eq!(response.page, 1);
				}
				state => panic!("expected the newer success state, got {state:?}"),
			}
		});
	}

	#[test]
	fn test_direct_list_route_extracts_model_name() {
		let html = ReactiveScope::run(|| {
			let router = init_router();

			router.push("/admin/question/").unwrap();
			router.render_current().render_to_string()
		});

		assert!(
			html.contains("question"),
			"Direct list route render should pass the model path segment to the view"
		);
	}

	#[test]
	fn test_json_value_to_display_string_preserves_non_string_scalars() {
		assert_eq!(json_value_to_display_string(&serde_json::json!(42)), "42");
		assert_eq!(
			json_value_to_display_string(&serde_json::json!(true)),
			"true"
		);
		assert_eq!(json_value_to_display_string(&serde_json::Value::Null), "");
		assert_eq!(
			json_value_to_display_string(&serde_json::json!([1, "two", false])),
			"1, two, false"
		);
	}

	#[rstest]
	fn list_response_mapping_preserves_edit_metadata_and_typed_values() {
		// Arrange
		let response = crate::types::ListResponse {
			model_name: "User".to_string(),
			pk_field: "slug".to_string(),
			count: 1,
			page: 1,
			page_size: 100,
			total_pages: 1,
			results: vec![HashMap::from([
				("slug".to_string(), serde_json::json!("alice")),
				("score".to_string(), serde_json::json!(42)),
				("active".to_string(), serde_json::json!(true)),
				("nickname".to_string(), serde_json::Value::Null),
			])],
			available_filters: None,
			columns: Some(vec![crate::types::ColumnInfo {
				field: "score".to_string(),
				label: "Score".to_string(),
				sortable: true,
				editable: true,
				linked: false,
				required: true,
				nullable: false,
				step: None,
				form_spec: Some(crate::types::FormFieldSpec::Input {
					html_type: "number".to_string(),
				}),
			}]),
		};

		// Act
		let data = list_response_to_view_data(response);

		// Assert
		assert_eq!(data.pk_field, "slug");
		assert!(data.columns[0].editable);
		assert!(data.columns[0].required);
		assert_eq!(data.records[0]["score"], serde_json::json!(42));
		assert_eq!(data.records[0]["active"], serde_json::json!(true));
		assert_eq!(data.records[0]["nickname"], serde_json::Value::Null);
	}

	#[test]
	fn test_detail_view_with_params() {
		let html = ReactiveScope::run(|| {
			let view = detail_view_component("users".to_string(), "42".to_string());
			view.render_to_string()
		});
		assert!(!html.is_empty());
	}

	// ==================== Spec-based tests for #3114 ====================

	/// Verify the admin SPA router has a login route.
	/// The WASM SPA must provide a login form for JWT authentication
	/// when the user is unauthenticated (#3114).
	#[test]
	fn test_admin_router_has_login_route() {
		// Act
		let has_login_route = ReactiveScope::run(|| init_router().has_route("login"));

		// Assert
		assert!(
			has_login_route,
			"Admin router must have a 'login' route for authentication flow. \
			 The SPA needs a login form to obtain JWT tokens (#3114)."
		);
	}

	/// Verify the /admin/login/ path matches the login route (#3114).
	#[test]
	fn test_login_route_match() {
		// Act
		let route_name = ReactiveScope::run(|| {
			let router = init_router();
			router
				.match_path("/admin/login/")
				.and_then(|route_match| route_match.route.name().map(str::to_owned))
		});

		// Assert
		assert!(
			route_name.is_some(),
			"Path /admin/login/ should match the login route (#3114)"
		);
		assert_eq!(route_name.as_deref(), Some("login"));
	}
}
