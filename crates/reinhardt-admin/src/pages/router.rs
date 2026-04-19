//! Client-side router for Reinhardt Admin Panel
//!
//! Handles routing between different admin pages:
//! - `/admin/login/` - Login form
//! - `/admin/` - Dashboard
//! - `/admin/{model}/` - List view
//! - `/admin/{model}/{id}/` - Detail view
//! - `/admin/{model}/add/` - Create form
//! - `/admin/{model}/{id}/change/` - Edit form

use crate::pages::components::features::{
	Column, FormField, ListViewData, dashboard, detail_view, list_view, model_form,
};
pub use crate::pages::components::login;
#[cfg(client)]
use crate::server::{get_dashboard, get_detail, get_fields, get_list};
#[cfg(client)]
use crate::types::ListQueryParams;
use crate::types::ModelInfo;
use reinhardt_pages::Signal;
use reinhardt_pages::component::{Component, Page};
use reinhardt_pages::page;
use reinhardt_pages::router::{Link, Router};
#[cfg(client)]
use reinhardt_pages::{ResourceState, create_resource};
use std::cell::RefCell;
use std::collections::HashMap;

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
thread_local! {
	static ROUTER: RefCell<Option<Router>> = const { RefCell::new(None) };
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
	ROUTER.with(|r| {
		*r.borrow_mut() = Some(init_router());
	});
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
	F: FnOnce(&Router) -> R,
{
	ROUTER.with(|r| r.borrow().as_ref().map(f))
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
	F: FnOnce(&Router) -> R,
{
	try_with_router(f).expect("Router not initialized. Call init_global_router() first.")
}

/// Dashboard view component for router
#[cfg(client)]
fn dashboard_view() -> Page {
	let dashboard_resource =
		create_resource(|| async { get_dashboard().await.map_err(|e| e.to_string()) });

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

	page!(|| {
		div {
			class: "dashboard-container p-6 md:p-8 max-w-7xl mx-auto",
			{ reactive_content }
		}
	})()
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
	use reinhardt_pages::use_effect;

	let list_resource = create_resource(move || {
		let model_name = model_name.clone();
		async move {
			let params = ListQueryParams::default();
			get_list(model_name, params)
				.await
				.map_err(|e| e.to_string())
		}
	});

	// Create signals outside the reactive closure so they persist across re-renders
	let page_signal = Signal::new(1u64);
	let filters_signal = Signal::new(HashMap::new());

	// Sync page_signal from the completed resource outside the rendering closure.
	// Updating signals inside a rendering closure is an anti-pattern: it causes
	// a state change during render and could create an infinite loop if the
	// resource ever reads page_signal. Using use_effect keeps side-effects
	// separate from the render path.
	{
		let resource = list_resource.clone();
		let page_signal = page_signal.clone();
		use_effect(move || {
			if let ResourceState::Success(ref response) = resource.get() {
				page_signal.set(response.page);
			}
		});
	}

	let reactive_content = Page::reactive({
		let resource = list_resource.clone();
		let page_signal = page_signal.clone();
		let filters_signal = filters_signal.clone();
		move || match resource.get() {
			ResourceState::Loading => loading_view(),
			ResourceState::Success(response) => {
				let data = ListViewData {
					model_name: response.model_name.clone(),
					columns: response
						.columns
						.map(|cols| {
							cols.into_iter()
								.map(|c| Column {
									field: c.field,
									label: c.label,
									sortable: c.sortable,
								})
								.collect()
						})
						.unwrap_or_else(|| {
							vec![Column {
								field: "id".to_string(),
								label: "ID".to_string(),
								sortable: true,
							}]
						}),
					records: response
						.results
						.into_iter()
						.map(|record| {
							record
								.into_iter()
								.map(|(k, v)| (k, v.as_str().unwrap_or("").to_string()))
								.collect()
						})
						.collect(),
					current_page: response.page,
					total_pages: response.total_pages,
					total_count: response.count,
					filters: response.available_filters.unwrap_or_default(),
				};
				list_view(&data, page_signal.clone(), filters_signal.clone())
			}
			ResourceState::Error(err) => error_view(&err),
		}
	});

	page!(|| {
		div {
			class: "list-container p-6 md:p-8 max-w-7xl mx-auto",
			{ reactive_content }
		}
	})()
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
			},
			Column {
				field: "name".to_string(),
				label: "Name".to_string(),
				sortable: true,
			},
		],
		records: vec![],
		current_page: 1,
		total_pages: 1,
		total_count: 0,
		filters: vec![],
	};

	let page_signal = Signal::new(1u64);
	let filters_signal = Signal::new(HashMap::new());
	list_view(&data, page_signal, filters_signal)
}

/// Detail view component for router
#[cfg(client)]
fn detail_view_component(model_name: String, record_id: String) -> Page {
	let model_name_for_view = model_name.clone();
	let record_id_for_view = record_id.clone();
	let detail_resource = create_resource(move || {
		let model_name = model_name.clone();
		let record_id = record_id.clone();
		async move {
			get_detail(model_name, record_id)
				.await
				.map_err(|e| e.to_string())
		}
	});

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
					.map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
					.collect();
				detail_view(&model_name, &record_id, &data)
			}
			ResourceState::Error(err) => error_view(&err),
		}
	});

	page!(|| {
		div {
			class: "detail-container p-6 md:p-8 max-w-7xl mx-auto",
			{ reactive_content }
		}
	})()
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

/// Create form view component for router
#[cfg(client)]
fn create_view_component(model_name: String) -> Page {
	let model_name_for_view = model_name.clone();
	let fields_resource = create_resource(move || {
		let model_name = model_name.clone();
		async move {
			get_fields(model_name, None)
				.await
				.map_err(|e| e.to_string())
		}
	});

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
						value: String::new(),
					})
					.collect();
				model_form(&model_name, &fields, None)
			}
			ResourceState::Error(err) => error_view(&err),
		}
	});

	page!(|| {
		div {
			class: "form-container p-6 md:p-8 max-w-7xl mx-auto",
			{ reactive_content }
		}
	})()
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
			value: String::new(),
		},
		FormField {
			name: "email".to_string(),
			label: "Email".to_string(),
			spec: crate::types::FormFieldSpec::Input {
				html_type: "email".to_string(),
			},
			required: true,
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
	let fields_resource = create_resource(move || {
		let model_name = model_name.clone();
		let record_id = record_id.clone();
		async move {
			get_fields(model_name, Some(record_id))
				.await
				.map_err(|e| e.to_string())
		}
	});

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
						let (value, values) = if let Some(ref vals) = response.values {
							match vals.get(&field_info.name) {
								// Multi-valued arrays become `values`
								Some(v) if v.is_array() => {
									let list: Vec<String> = v
										.as_array()
										.map(|arr| {
											arr.iter()
												.filter_map(|x| x.as_str().map(|s| s.to_string()))
												.collect()
										})
										.unwrap_or_default();
									(String::new(), list)
								}
								Some(v) => (v.as_str().unwrap_or("").to_string(), Vec::new()),
								None => (String::new(), Vec::new()),
							}
						} else {
							(String::new(), Vec::new())
						};

						FormField {
							spec: crate::types::FormFieldSpec::from(&field_info.field_type),
							name: field_info.name,
							label: field_info.label,
							required: field_info.required,
							value,
							values,
						}
					})
					.collect();
				model_form(&model_name, &fields, Some(&record_id))
			}
			ResourceState::Error(err) => error_view(&err),
		}
	});

	page!(|| {
		div {
			class: "form-container p-6 md:p-8 max-w-7xl mx-auto",
			{ reactive_content }
		}
	})()
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
			value: "Existing Value".to_string(),
		},
		FormField {
			name: "email".to_string(),
			label: "Email".to_string(),
			spec: crate::types::FormFieldSpec::Input {
				html_type: "email".to_string(),
			},
			required: true,
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

	page!(|| {
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
			div {
				{ dashboard_link }
			}
		}
	})()
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
	use reinhardt_pages::component::IntoPage;

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

	page!(|| {
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
	})()
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
/// 4. `/admin/{model}/{id}/` - detail (all dynamic segments)
/// 5. `/admin/{model}/` - list (all dynamic segments)
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
pub fn init_router() -> Router {
	// IMPORTANT: Route registration order matters. See doc comment above.
	// Login route must be registered before dynamic routes to prevent
	// /admin/login/ from matching the list route with model="login".
	Router::new()
		.named_route("login", "/admin/login/", login::login_view)
		.named_route("dashboard", "/admin/", dashboard_view)
		.named_route("create", "/admin/{model}/add/", || {
			with_router(|router| {
				let params = router.current_params().get();
				let model_name = params
					.get("model")
					.cloned()
					.unwrap_or_else(|| "unknown".to_string());
				create_view_component(model_name)
			})
		})
		.named_route("edit", "/admin/{model}/{id}/change/", || {
			with_router(|router| {
				let params = router.current_params().get();
				let model_name = params
					.get("model")
					.cloned()
					.unwrap_or_else(|| "unknown".to_string());
				let record_id = params.get("id").cloned().unwrap_or_else(|| "0".to_string());
				edit_view_component(model_name, record_id)
			})
		})
		.named_route("detail", "/admin/{model}/{id}/", || {
			with_router(|router| {
				let params = router.current_params().get();
				let model_name = params
					.get("model")
					.cloned()
					.unwrap_or_else(|| "unknown".to_string());
				let record_id = params.get("id").cloned().unwrap_or_else(|| "0".to_string());
				detail_view_component(model_name, record_id)
			})
		})
		.named_route("list", "/admin/{model}/", || {
			with_router(|router| {
				let params = router.current_params().get();
				let model_name = params
					.get("model")
					.cloned()
					.unwrap_or_else(|| "unknown".to_string());
				list_view_component(model_name)
			})
		})
		.not_found(not_found_view)
}

#[cfg(test)]
mod tests {
	use super::*;

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
		let router = init_router();
		assert_eq!(router.route_count(), 6); // login + dashboard + list + detail + create + edit
		assert!(router.has_route("login"));
		assert!(router.has_route("dashboard"));
		assert!(router.has_route("list"));
		assert!(router.has_route("detail"));
		assert!(router.has_route("create"));
		assert!(router.has_route("edit"));
	}

	#[test]
	fn test_dashboard_route_match() {
		let router = init_router();
		let route_match = router.match_path("/admin/");
		assert!(route_match.is_some());

		let route_match = route_match.unwrap();
		assert_eq!(route_match.route.name(), Some("dashboard"));
	}

	#[test]
	fn test_list_route_match() {
		let router = init_router();
		let route_match = router.match_path("/admin/users/");
		assert!(route_match.is_some());

		let route_match = route_match.unwrap();
		assert_eq!(route_match.route.name(), Some("list"));
		assert_eq!(route_match.params.get("model"), Some(&"users".to_string()));
	}

	#[test]
	fn test_detail_route_match() {
		let router = init_router();
		let route_match = router.match_path("/admin/users/42/");
		assert!(route_match.is_some());

		let route_match = route_match.unwrap();
		assert_eq!(route_match.route.name(), Some("detail"));
		assert_eq!(route_match.params.get("model"), Some(&"users".to_string()));
		assert_eq!(route_match.params.get("id"), Some(&"42".to_string()));
	}

	#[test]
	fn test_create_route_match() {
		let router = init_router();
		let route_match = router.match_path("/admin/users/add/");
		assert!(route_match.is_some());

		let route_match = route_match.unwrap();
		assert_eq!(route_match.route.name(), Some("create"));
		assert_eq!(route_match.params.get("model"), Some(&"users".to_string()));
	}

	#[test]
	fn test_edit_route_match() {
		let router = init_router();
		let route_match = router.match_path("/admin/users/42/change/");
		assert!(route_match.is_some());

		let route_match = route_match.unwrap();
		assert_eq!(route_match.route.name(), Some("edit"));
		assert_eq!(route_match.params.get("model"), Some(&"users".to_string()));
		assert_eq!(route_match.params.get("id"), Some(&"42".to_string()));
	}

	#[test]
	fn test_reverse_url_dashboard() {
		let router = init_router();
		let url = router.reverse("dashboard", &[]).unwrap();
		assert_eq!(url, "/admin/");
	}

	#[test]
	fn test_reverse_url_list() {
		let router = init_router();
		let url = router.reverse("list", &[("model", "users")]).unwrap();
		assert_eq!(url, "/admin/users/");
	}

	#[test]
	fn test_reverse_url_detail() {
		let router = init_router();
		let url = router
			.reverse("detail", &[("model", "users"), ("id", "42")])
			.unwrap();
		assert_eq!(url, "/admin/users/42/");
	}

	#[test]
	fn test_init_global_router() {
		init_global_router();

		with_router(|router| {
			assert_eq!(router.route_count(), 6);
			assert!(router.has_route("login"));
			assert!(router.has_route("dashboard"));
			assert!(router.has_route("list"));
			assert!(router.has_route("detail"));
			assert!(router.has_route("create"));
			assert!(router.has_route("edit"));
		});
	}

	#[test]
	fn test_with_router_access() {
		init_global_router();

		let route_count = with_router(|router| router.route_count());
		assert_eq!(route_count, 6);

		let has_dashboard = with_router(|router| router.has_route("dashboard"));
		assert!(has_dashboard);
	}

	#[test]
	#[should_panic(expected = "Router not initialized")]
	fn test_with_router_panics_when_not_initialized() {
		// Clear ROUTER (this operation is actually dangerous, but for test purposes)
		ROUTER.with(|r| *r.borrow_mut() = None);

		with_router(|_| {});
	}

	#[test]
	fn test_try_with_router_returns_none_when_not_initialized() {
		// Clear ROUTER to simulate uninitialized state
		ROUTER.with(|r| *r.borrow_mut() = None);

		let result = try_with_router(|router| router.route_count());
		assert!(result.is_none());
	}

	#[test]
	fn test_try_with_router_returns_some_when_initialized() {
		init_global_router();

		let result = try_with_router(|router| router.route_count());
		assert_eq!(result, Some(6));
	}

	#[test]
	fn test_list_view_with_model_name() {
		let view = list_view_component("users".to_string());
		// Verify basic rendering succeeds
		let html = view.render_to_string();
		assert!(html.contains("users") || html.contains("List"));
	}

	#[test]
	fn test_detail_view_with_params() {
		let view = detail_view_component("users".to_string(), "42".to_string());
		// Verify basic rendering succeeds
		let html = view.render_to_string();
		assert!(!html.is_empty());
	}

	// ==================== Spec-based tests for #3114 ====================

	/// Verify the admin SPA router has a login route.
	/// The WASM SPA must provide a login form for JWT authentication
	/// when the user is unauthenticated (#3114).
	#[test]
	fn test_admin_router_has_login_route() {
		// Arrange & Act
		let router = init_router();

		// Assert - a login route must exist for the auth flow
		assert!(
			router.has_route("login"),
			"Admin router must have a 'login' route for authentication flow. \
			 The SPA needs a login form to obtain JWT tokens (#3114)."
		);
	}

	/// Verify the /admin/login/ path matches the login route (#3114).
	#[test]
	fn test_login_route_match() {
		// Arrange
		let router = init_router();

		// Act
		let route_match = router.match_path("/admin/login/");

		// Assert
		assert!(
			route_match.is_some(),
			"Path /admin/login/ should match the login route (#3114)"
		);
		let route_match = route_match.unwrap();
		assert_eq!(route_match.route.name(), Some("login"));
	}
}
