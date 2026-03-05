//! Client-side router for Reinhardt Admin Panel
//!
//! Handles routing between different admin pages:
//! - `/admin/` - Dashboard
//! - `/admin/{model}/` - List view
//! - `/admin/{model}/{id}/` - Detail view
//! - `/admin/{model}/add/` - Create form
//! - `/admin/{model}/{id}/change/` - Edit form

use crate::pages::components::features::{
	Column, FormField, ListViewData, dashboard, detail_view, list_view, model_form,
};
#[cfg(target_arch = "wasm32")]
use crate::server::{get_dashboard, get_model_detail, get_model_fields, list_models};
#[cfg(target_arch = "wasm32")]
use crate::types::ListQueryParams;
use crate::types::ModelInfo;
use reinhardt_pages::Signal;
use reinhardt_pages::component::{Component, Page};
use reinhardt_pages::router::{Link, Router};
#[cfg(target_arch = "wasm32")]
use reinhardt_pages::{ResourceState, create_resource};
use std::cell::RefCell;
use std::collections::HashMap;

/// Admin route enum
#[derive(Debug, Clone, PartialEq)]
pub enum AdminRoute {
	/// Dashboard route
	Dashboard,
	/// List view route
	List { model_name: String },
	/// Detail view route
	Detail { model_name: String, id: String },
	/// Create form route
	Create { model_name: String },
	/// Edit form route
	Edit { model_name: String, id: String },
	/// Not found route
	NotFound,
}

// Global Router instance
// Initialized by init_global_router() and accessed via with_router()
thread_local! {
	static ROUTER: RefCell<Option<Router>> = const { RefCell::new(None) };
}

/// Initialize the global router instance
///
/// This must be called once at application startup before any routing operations.
///
/// # Example
///
/// ```ignore
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
/// ```ignore
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
/// ```ignore
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
#[cfg(target_arch = "wasm32")]
fn dashboard_view() -> Page {
	use reinhardt_pages::component::{IntoPage, PageElement};

	let dashboard_resource =
		create_resource(|| async { get_dashboard().await.map_err(|e| e.to_string()) });

	PageElement::new("div")
		.attr("class", "dashboard-container")
		.child({
			let resource = dashboard_resource.clone();
			move || match resource.get() {
				ResourceState::Loading => loading_view(),
				ResourceState::Success(data) => dashboard(&data.site_name, &data.models),
				ResourceState::Error(err) => error_view(&err),
			}
		})
		.into_page()
}

/// Dashboard view component for router (non-WASM fallback)
#[cfg(not(target_arch = "wasm32"))]
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

	dashboard("Admin Panel", &models)
}

/// List view component for router
#[cfg(target_arch = "wasm32")]
fn list_view_component(model_name: String) -> Page {
	use reinhardt_pages::component::{IntoPage, PageElement};

	let list_resource = create_resource(move || {
		let model_name = model_name.clone();
		async move {
			let params = ListQueryParams::default();
			list_models(model_name, params)
				.await
				.map_err(|e| e.to_string())
		}
	});

	PageElement::new("div")
		.attr("class", "list-container")
		.child({
			let resource = list_resource.clone();
			move || match resource.get() {
				ResourceState::Loading => loading_view(),
				ResourceState::Success(response) => {
					use std::collections::HashMap;

					// Convert ListResponse to ListViewData
					let data = ListViewData {
						model_name: response.model_name.clone(),
						columns: response.columns.unwrap_or_else(|| {
							vec![Column {
								field: "id".to_string(),
								label: "ID".to_string(),
								sortable: true,
							}]
						}),
						records: response.results,
						current_page: response.page,
						total_pages: response.total_pages,
						total_count: response.count,
						filters: response.available_filters.unwrap_or_default(),
					};
					let page_signal = Signal::new(response.page);
					let filters_signal = Signal::new(HashMap::new());
					list_view(&data, page_signal, filters_signal)
				}
				ResourceState::Error(err) => error_view(&err),
			}
		})
		.into_page()
}

/// List view component for router (non-WASM fallback)
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(target_arch = "wasm32")]
fn detail_view_component(model_name: String, record_id: String) -> Page {
	use reinhardt_pages::component::{IntoPage, PageElement};

	let detail_resource = create_resource(move || {
		let model_name = model_name.clone();
		let record_id = record_id.clone();
		async move {
			get_model_detail(model_name, record_id)
				.await
				.map_err(|e| e.to_string())
		}
	});

	PageElement::new("div")
		.attr("class", "detail-container")
		.child({
			let resource = detail_resource.clone();
			let model_name = model_name.clone();
			let record_id = record_id.clone();
			move || match resource.get() {
				ResourceState::Loading => loading_view(),
				ResourceState::Success(response) => {
					detail_view(&model_name, &record_id, &response.data)
				}
				ResourceState::Error(err) => error_view(&err),
			}
		})
		.into_page()
}

/// Detail view component for router (non-WASM fallback)
#[cfg(not(target_arch = "wasm32"))]
fn detail_view_component(model_name: String, record_id: String) -> Page {
	// Dummy data for non-WASM environments (tests, etc.)
	let mut record = HashMap::new();
	record.insert("id".to_string(), record_id.clone());
	record.insert("name".to_string(), "Sample Record".to_string());

	detail_view(&model_name, &record_id, &record)
}

/// Create form view component for router
#[cfg(target_arch = "wasm32")]
fn create_view_component(model_name: String) -> Page {
	use reinhardt_pages::component::{IntoPage, PageElement};

	let fields_resource = create_resource(move || {
		let model_name = model_name.clone();
		async move {
			get_model_fields(model_name, None)
				.await
				.map_err(|e| e.to_string())
		}
	});

	PageElement::new("div")
		.attr("class", "form-container")
		.child({
			let resource = fields_resource.clone();
			let model_name = model_name.clone();
			move || match resource.get() {
				ResourceState::Loading => loading_view(),
				ResourceState::Success(response) => {
					// Convert FieldInfo to FormField
					let fields: Vec<FormField> = response
						.fields
						.into_iter()
						.map(|field_info| FormField {
							name: field_info.name,
							label: field_info.label,
							field_type: field_type_to_html_input_type(&field_info.field_type),
							required: field_info.required,
							value: String::new(),
						})
						.collect();
					model_form(&model_name, &fields, None)
				}
				ResourceState::Error(err) => error_view(&err),
			}
		})
		.into_page()
}

/// Create form view component for router (non-WASM fallback)
#[cfg(not(target_arch = "wasm32"))]
fn create_view_component(model_name: String) -> Page {
	// Dummy data for non-WASM environments (tests, etc.)
	let fields = vec![
		FormField {
			name: "name".to_string(),
			label: "Name".to_string(),
			field_type: "text".to_string(),
			required: true,
			value: String::new(),
		},
		FormField {
			name: "email".to_string(),
			label: "Email".to_string(),
			field_type: "email".to_string(),
			required: true,
			value: String::new(),
		},
	];

	model_form(&model_name, &fields, None)
}

/// Edit form view component for router
#[cfg(target_arch = "wasm32")]
fn edit_view_component(model_name: String, record_id: String) -> Page {
	use reinhardt_pages::component::{IntoPage, PageElement};

	let fields_resource = create_resource(move || {
		let model_name = model_name.clone();
		let record_id = record_id.clone();
		async move {
			get_model_fields(model_name, Some(record_id))
				.await
				.map_err(|e| e.to_string())
		}
	});

	PageElement::new("div")
		.attr("class", "form-container")
		.child({
			let resource = fields_resource.clone();
			let model_name = model_name.clone();
			let record_id = record_id.clone();
			move || match resource.get() {
				ResourceState::Loading => loading_view(),
				ResourceState::Success(response) => {
					// Convert FieldInfo + values to FormField
					let fields: Vec<FormField> = response
						.fields
						.into_iter()
						.map(|field_info| {
							// Get existing value
							let value = if let Some(ref values) = response.values {
								values
									.get(&field_info.name)
									.and_then(|v| v.as_str())
									.unwrap_or("")
									.to_string()
							} else {
								String::new()
							};

							FormField {
								name: field_info.name,
								label: field_info.label,
								field_type: field_type_to_html_input_type(&field_info.field_type),
								required: field_info.required,
								value,
							}
						})
						.collect();
					model_form(&model_name, &fields, Some(&record_id))
				}
				ResourceState::Error(err) => error_view(&err),
			}
		})
		.into_page()
}

/// Edit form view component for router (non-WASM fallback)
#[cfg(not(target_arch = "wasm32"))]
fn edit_view_component(model_name: String, record_id: String) -> Page {
	// Dummy data for non-WASM environments (tests, etc.)
	let fields = vec![
		FormField {
			name: "name".to_string(),
			label: "Name".to_string(),
			field_type: "text".to_string(),
			required: true,
			value: "Existing Value".to_string(),
		},
		FormField {
			name: "email".to_string(),
			label: "Email".to_string(),
			field_type: "email".to_string(),
			required: true,
			value: "user@example.com".to_string(),
		},
	];

	model_form(&model_name, &fields, Some(&record_id))
}

/// Not found view component for router
fn not_found_view() -> Page {
	use reinhardt_pages::component::{IntoPage, PageElement};

	PageElement::new("div")
		.attr("class", "not-found")
		.child(
			PageElement::new("h1")
				.attr("class", "text-center mt-5")
				.child("404 - Page Not Found"),
		)
		.child(
			PageElement::new("p")
				.attr("class", "text-center")
				.child("The requested page could not be found."),
		)
		.child(
			PageElement::new("div")
				.attr("class", "text-center mt-3")
				.child(Link::new("/admin/", "Go to Dashboard").render()),
		)
		.into_page()
}

/// Loading view component
///
/// Displays a loading indicator while data is being fetched.
#[cfg(target_arch = "wasm32")]
fn loading_view() -> Page {
	use reinhardt_pages::component::{IntoPage, PageElement};

	PageElement::new("div")
		.attr("class", "loading-spinner text-center mt-5")
		.child(
			PageElement::new("div")
				.attr("class", "spinner-border")
				.attr("role", "status")
				.child(
					PageElement::new("span")
						.attr("class", "visually-hidden")
						.child("Loading..."),
				),
		)
		.into_page()
}

/// Error view component
///
/// Displays an error message when data fetch fails.
#[cfg(target_arch = "wasm32")]
fn error_view(message: &str) -> Page {
	use reinhardt_pages::component::{IntoPage, PageElement};

	PageElement::new("div")
		.attr("class", "error-message alert alert-danger mt-5")
		.attr("role", "alert")
		.child(
			PageElement::new("h4")
				.attr("class", "alert-heading")
				.child("Error"),
		)
		.child(PageElement::new("p").child(message.to_string()))
		.child(
			PageElement::new("div")
				.attr("class", "mt-3")
				.child(Link::new("/admin/", "Go to Dashboard").render()),
		)
		.into_page()
}

/// Convert FieldType to HTML input type string
///
/// Maps reinhardt_admin::types::FieldType to HTML input type attributes.
#[cfg(target_arch = "wasm32")]
fn field_type_to_html_input_type(field_type: &reinhardt_admin::types::FieldType) -> String {
	use reinhardt_admin::types::FieldType;

	match field_type {
		FieldType::Text => "text".to_string(),
		FieldType::TextArea => "textarea".to_string(),
		FieldType::Number => "number".to_string(),
		FieldType::Boolean => "checkbox".to_string(),
		FieldType::Email => "email".to_string(),
		FieldType::Date => "date".to_string(),
		FieldType::DateTime => "datetime-local".to_string(),
		FieldType::Select { .. } => "select".to_string(),
		FieldType::MultiSelect { .. } => "select-multiple".to_string(),
		FieldType::File => "file".to_string(),
		FieldType::Hidden => "hidden".to_string(),
	}
}

/// Initialize the admin router
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::pages::router::init_router;
///
/// let router = init_router();
/// ```
pub fn init_router() -> Router {
	Router::new()
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
		assert_eq!(router.route_count(), 5); // dashboard + list + detail + create + edit
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
			assert_eq!(router.route_count(), 5);
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
		assert_eq!(route_count, 5);

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
		assert_eq!(result, Some(5));
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
}
