//! Login component for Reinhardt Admin Panel
//!
//! Provides a login form that authenticates admin users via JWT.

use reinhardt_pages::component::{IntoPage, Page, PageElement};
use reinhardt_pages::page;

#[cfg(target_arch = "wasm32")]
use reinhardt_pages::Signal;

/// Login form component
///
/// Renders a login form with username and password fields.
/// On successful authentication, stores the JWT token in sessionStorage
/// and updates the reactive auth state.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::pages::components::login::login_form;
///
/// let page = login_form(None);
/// ```
pub fn login_form(error_message: Option<&str>) -> Page {
	let error_html = error_message.map(|msg| {
		PageElement::new("div")
			.attr(
				"class",
				"admin-alert admin-alert-danger mt-4 text-center text-sm",
			)
			.attr("role", "alert")
			.child(msg.to_string())
			.into_page()
	});

	let form_page = build_login_form();

	let mut container = PageElement::new("div")
		.attr(
			"class",
			"flex justify-center items-center min-h-screen bg-slate-50",
		)
		.child(
			PageElement::new("div")
				.attr("class", "admin-login-card")
				.child(
					PageElement::new("div")
						.attr("class", "p-8")
						.child(
							PageElement::new("h2")
								.attr(
									"class",
									"font-display text-2xl font-bold text-center mb-1 text-slate-900",
								)
								.child("Admin Login"),
						)
						.child(
							PageElement::new("p")
								.attr("class", "text-sm text-slate-500 text-center mb-6")
								.child("Sign in to manage your application"),
						)
						.child(form_page),
				),
		);

	if let Some(err) = error_html {
		container = container.child(err);
	}

	container.into_page()
}

/// Builds the login form HTML structure.
fn build_login_form() -> Page {
	page!(|| {
		form {
			id: "admin-login-form",
			method: "post",
			div {
				class: "mb-4",
				label {
					r#for: "username",
					class: "admin-label",
					"Username"
				}
				input {
					r#type: "text",
					class: "admin-input",
					id: "username",
					name: "username",
					required: true,
					autocomplete: "username",
					autofocus: true,
					placeholder: "Enter your username",
				}
			}
			div {
				class: "mb-5",
				label {
					r#for: "password",
					class: "admin-label",
					"Password"
				}
				input {
					r#type: "password",
					class: "admin-input",
					id: "password",
					name: "password",
					required: true,
					autocomplete: "current-password",
					placeholder: "Enter your password",
				}
			}
			div {
				id: "login-error",
				class: "admin-alert admin-alert-danger hidden mb-4",
				role: "alert",
			}
			button {
				r#type: "submit",
				class: "admin-btn admin-btn-primary w-full py-2.5 text-base",
				id: "login-submit-btn",
				"Sign in"
			}
		}
	})()
}

/// Login view component for the WASM router.
///
/// On WASM targets, this sets up an event handler that intercepts form
/// submission, calls the `admin_login` server function, and handles
/// the authentication flow (token storage, auth state update, redirect).
#[cfg(target_arch = "wasm32")]
pub fn login_view() -> Page {
	use crate::server::login::admin_login;
	use reinhardt_pages::auth::{auth_state, set_jwt_token};
	use reinhardt_pages::component::PageElement;
	use reinhardt_pages::csrf::get_csrf_token;
	use wasm_bindgen::JsCast;
	use wasm_bindgen::prelude::*;

	let error_signal = Signal::new(Option::<String>::None);

	PageElement::new("div")
		.attr("class", "login-wrapper")
		.child(Page::reactive({
			let error_signal = error_signal.clone();
			move || {
				let error = error_signal.get();
				login_form(error.as_deref())
			}
		}))
		.attr("data-login-view", "true")
		.into_page()
}

/// Login view component for non-WASM targets (static form rendering).
#[cfg(not(target_arch = "wasm32"))]
pub fn login_view() -> Page {
	login_form(None)
}

/// Sets up the login form submission handler.
///
/// This function is called after the login view is mounted to the DOM.
/// It attaches an event listener to the form that intercepts submission,
/// calls the server function, and handles the response.
#[cfg(target_arch = "wasm32")]
pub fn setup_login_handler() {
	use crate::server::login::admin_login;
	use reinhardt_pages::auth::{auth_state, set_jwt_token};
	use reinhardt_pages::csrf::get_csrf_token;
	use wasm_bindgen::JsCast;
	use wasm_bindgen::prelude::*;
	use wasm_bindgen_futures::spawn_local;
	use web_sys::{Event, HtmlInputElement, window};

	let window = match window() {
		Some(w) => w,
		None => return,
	};
	let document = match window.document() {
		Some(d) => d,
		None => return,
	};
	let form = match document.get_element_by_id("admin-login-form") {
		Some(f) => f,
		None => return,
	};

	let handler = Closure::wrap(Box::new(move |event: Event| {
		event.prevent_default();

		let window = match web_sys::window() {
			Some(w) => w,
			None => return,
		};
		let document = match window.document() {
			Some(d) => d,
			None => return,
		};

		// Get form values
		let username = document
			.get_element_by_id("username")
			.and_then(|el| el.dyn_into::<HtmlInputElement>().ok())
			.map(|el| el.value())
			.unwrap_or_default();

		let password = document
			.get_element_by_id("password")
			.and_then(|el| el.dyn_into::<HtmlInputElement>().ok())
			.map(|el| el.value())
			.unwrap_or_default();

		let csrf_token = get_csrf_token().unwrap_or_default();

		// Disable submit button during request
		if let Some(btn) = document.get_element_by_id("login-submit-btn") {
			let _ = btn.set_attribute("disabled", "true");
			btn.set_text_content(Some("Signing in..."));
		}

		// Hide previous error
		if let Some(error_div) = document.get_element_by_id("login-error") {
			let _ = error_div.class_list().add_1("hidden");
		}

		spawn_local(async move {
			match admin_login(username, password, csrf_token).await {
				Ok(response) => {
					// Store JWT token
					set_jwt_token(&response.token);

					// Update reactive auth state
					let auth = auth_state();
					auth.login_full(
						response.user_id.parse::<i64>().unwrap_or(0),
						&response.username,
						None,
						response.is_staff,
						response.is_superuser,
					);

					// Navigate to dashboard
					crate::pages::router::with_router(|r| {
						let _ = r.push("/admin/");
					});
				}
				Err(e) => {
					let error_msg = e.to_string();
					let window = web_sys::window();
					let document = window.as_ref().and_then(|w| w.document());

					if let Some(doc) = document {
						// Show error message
						if let Some(error_div) = doc.get_element_by_id("login-error") {
							let _ = error_div.class_list().remove_1("hidden");
							error_div.set_text_content(Some(if error_msg.contains("401") {
								"Invalid username or password"
							} else {
								"Login failed. Please try again."
							}));
						}

						// Re-enable submit button
						if let Some(btn) = doc.get_element_by_id("login-submit-btn") {
							let _ = btn.remove_attribute("disabled");
							btn.set_text_content(Some("Sign in"));
						}
					}
				}
			}
		});
	}) as Box<dyn FnMut(_)>);

	let _ = form.add_event_listener_with_callback("submit", handler.as_ref().unchecked_ref());
	handler.forget();
}

/// Sets up the login form submission handler (non-WASM stub).
#[cfg(not(target_arch = "wasm32"))]
pub fn setup_login_handler() {
	// No-op on non-WASM targets
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_login_form_renders() {
		// Arrange & Act
		let page = login_form(None);
		let html = page.render_to_string();

		// Assert
		assert!(html.contains("Admin Login"));
		assert!(html.contains("username"));
		assert!(html.contains("password"));
		assert!(html.contains("Sign in"));
	}

	#[rstest]
	fn test_login_form_with_error() {
		// Arrange & Act
		let page = login_form(Some("Invalid credentials"));
		let html = page.render_to_string();

		// Assert
		assert!(html.contains("Invalid credentials"));
		assert!(html.contains("admin-alert-danger"));
	}

	#[rstest]
	fn test_login_view_renders() {
		// Arrange & Act
		let page = login_view();
		let html = page.render_to_string();

		// Assert
		assert!(html.contains("admin-login-form"));
	}
}
