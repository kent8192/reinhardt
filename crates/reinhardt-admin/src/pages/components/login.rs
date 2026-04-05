//! Login component for Reinhardt Admin Panel
//!
//! Provides a login form that authenticates admin users via JWT.

use reinhardt_pages::component::Page;
use reinhardt_pages::form;
use reinhardt_pages::page;
// Used in form! macro closure type annotations (WASM-only codegen)
#[cfg(client)]
use crate::types::responses::LoginResponse;
#[cfg(client)]
use reinhardt_pages::ServerFnError;

/// Login form component
///
/// Renders a login form with username and password fields.
/// On successful authentication, the server sets a JWT HTTP-Only cookie
/// and the client updates the reactive auth state.
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
		let msg = msg.to_string();
		page!(|| {
			div {
				class: "admin-alert admin-alert-danger mt-4 text-center text-sm",
				role: "alert",
				{ msg }
			}
		})()
	});

	let form_page = build_login_form();
	let error_page = error_html.unwrap_or_else(|| page!(|| { span {} })());

	page!(|| {
		div {
			class: "flex justify-center items-center min-h-screen bg-slate-50 animate__animated animate__fadeIn",
			div {
				class: "admin-login-card",
				div {
					class: "p-8",
					h2 {
						class: "font-display text-2xl font-bold text-center mb-1 text-slate-900",
						"Admin Login"
					}
					p {
						class: "text-sm text-slate-500 text-center mb-6",
						"Sign in to manage your application"
					}
					{ form_page }
				}
			}
			{ error_page }
		}
	})()
}

/// Builds the login form HTML structure using the `form!` macro.
///
/// The struct name `AdminLoginForm` generates `id="admin-login-form"` on the
/// form element. The `server_fn: admin_login` directive auto-generates the
/// submit handler, replacing the manual `setup_login_handler()`.
///
/// The `on_success` callback updates the auth state and navigates to the
/// dashboard. JWT token storage is handled server-side via HTTP-Only cookie,
/// so no client-side token storage is needed.
fn build_login_form() -> Page {
	#[allow(unused_imports)]
	use crate::server::login::admin_login;

	let login_form = form! {
		name: AdminLoginForm,
		server_fn: admin_login,
		method: Post,

		fields: {
			username: CharField {
				required,
				label: "Username",
				label_class: "admin-label",
				wrapper_class: "mb-4",
				class: "admin-input",
				autocomplete: "username",
				autofocus,
				placeholder: "Enter your username",
			},
			password: CharField {
				required,
				widget: PasswordInput,
				label: "Password",
				label_class: "admin-label",
				wrapper_class: "mb-5",
				class: "admin-input",
				autocomplete: "current-password",
				placeholder: "Enter your password",
			},
		},

		on_success: |response: LoginResponse| {
			use reinhardt_pages::auth::auth_state;

			// JWT token is set as HTTP-Only cookie by the server.
			// No need to store in sessionStorage — browser handles it.

			let auth = auth_state();
			auth.login_full(
				response.user_id.clone(),
				&response.username,
				None,
				response.is_staff,
				response.is_superuser,
			);

			crate::pages::router::with_router(|r| {
				let _ = r.push("/admin/");
			});
		},

		on_error: |e: ServerFnError| {
			let error_msg = e.to_string();
			if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
				if let Some(error_div) = doc.get_element_by_id("login-error") {
					let _ = error_div.class_list().remove_1("hidden");
					error_div.set_text_content(Some(if error_msg.contains("401") {
						"Invalid username or password"
					} else {
						"Login failed. Please try again."
					}));
				}
			}
		},

		slots: {
			after_fields: || {
				page!(|| {
					div {
						id: "login-error",
						class: "admin-alert admin-alert-danger hidden mb-4",
						role: "alert",
					}
					button {
						type: "submit",
						class: "admin-btn admin-btn-primary w-full py-2.5 text-base",
						id: "login-submit-btn",
						"Sign in"
					}
				})()
			},
		},
	};

	login_form.into_page()
}

/// Login view component.
///
/// The `form!` macro with `server_fn: admin_login` auto-generates the submit
/// handler, so no separate `setup_login_handler()` is needed.
pub fn login_view() -> Page {
	login_form(None)
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
