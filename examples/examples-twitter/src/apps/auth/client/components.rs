//! Authentication components using form! macro
//!
//! Provides login and registration form components with form! macro for:
//! - Declarative field definitions with reactive Signal management
//! - UI state management (loading, error)
//! - Two-way binding (bind: true by default)
//! - Server function integration for form submission
//! - Automatic redirect on success

use reinhardt::pages::component::View;
use reinhardt::pages::form;
use reinhardt::pages::page;
use reinhardt::pages::reactive::Signal;

#[cfg(client)]
use {
	crate::apps::auth::client::state::set_current_user,
	crate::apps::auth::server::server_fn::{login, register},
};

/// Login form component using form! macro
///
/// Uses the `form!` macro for:
/// - Declarative field definitions (email, password)
/// - UI state management via `state: { loading, error }`
/// - Two-way binding via `bind: true` (default)
/// - SVG icons via `icon` property
/// - Server function integration via `server_fn`
/// - `on_success` callback for setting current user
/// - `redirect_on_success` for navigation after login
pub fn login_form() -> View {
	// Define form with state management and field definitions
	let login_form = form! {
		name: LoginForm,
		server_fn: login,

		// UI state management
		state: { loading, error },

		// Success callback: set current user before redirect
		on_success: |user_info| {
			#[cfg(client)]
			{
				set_current_user(Some(user_info));
			}
		},

		// Redirect after successful login
		redirect_on_success: "/timeline",

		fields: {
			email: EmailField {
				label: "Email",
				placeholder: "you@example.com",
				wrapper: div { class: "relative" },
				icon: svg {
					class: "w-5 h-5 text-content-tertiary",
					fill: "none",
					stroke: "currentColor",
					viewBox: "0 0 24 24",
					path {
						stroke_linecap: "round",
						stroke_linejoin: "round",
						stroke_width: "2",
						d: "M16 12a4 4 0 10-8 0 4 4 0 008 0zm0 0v1.5a2.5 2.5 0 005 0V12a9 9 0 10-9 9m4.5-1.206a8.959 8.959 0 01-4.5 1.207"
					}
				},
				icon_position: "left",
				class: "form-input pl-10",
			},
			password: PasswordField {
				label: "Password",
				placeholder: "Enter your password",
				wrapper: div { class: "relative" },
				icon: svg {
					class: "w-5 h-5 text-content-tertiary",
					fill: "none",
					stroke: "currentColor",
					viewBox: "0 0 24 24",
					path {
						stroke_linecap: "round",
						stroke_linejoin: "round",
						stroke_width: "2",
						d: "M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"
					}
				},
				icon_position: "left",
				class: "form-input pl-10",
			},
		},
	};

	// Clone state signals for page! macro
	let loading_signal = login_form.loading().clone();
	let error_signal = login_form.error().clone();

	// Convert form! to View before passing to page!
	let form_view = login_form.into_view();

	// Render custom UI using page! macro
	page!(|loading_signal: Signal<bool>, error_signal: Signal<Option<String>>, form_view: View| {
		div {
			class: "min-h-screen flex items-center justify-center px-4 py-12 bg-surface-secondary",
			div {
				class: "w-full max-w-md",
				div {
					class: "text-center mb-8",
					div {
						class: "inline-flex items-center justify-center w-16 h-16 rounded-full bg-brand/10 mb-4",
						svg {
							class: "w-8 h-8 text-brand",
							fill: "none",
							stroke: "currentColor",
							viewBox: "0 0 24 24",
							path {
								stroke_linecap: "round",
								stroke_linejoin: "round",
								stroke_width: "2",
								d: "M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z",
							}
						}
					}
					h1 {
						class: "text-2xl font-bold text-content-primary",
						"Welcome back"
					}
					p {
						class: "text-content-secondary mt-2",
						"Sign in to your account"
					}
				}
				div {
					class: "card animate-fade-in",
					div {
						class: "card-body p-6 sm:p-8",
						watch {
							if error_signal.get().is_some() {
								div {
									class: "alert-danger mb-4",
									div {
										class: "flex items-center gap-2",
										svg {
											class: "w-5 h-5 flex-shrink-0",
											fill: "currentColor",
											viewBox: "0 0 20 20",
											path {
												fill_rule: "evenodd",
												d: "M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z",
											}
										}
										span {
											{ error_signal.get().unwrap_or_default() }
										}
									}
								}
							}
						}
						{ form_view }
						div {
							class: "flex items-center justify-between mt-4",
							label {
								class: "flex items-center gap-2 cursor-pointer",
								input {
									r#type: "checkbox",
									class: "w-4 h-4 rounded border-border text-brand focus:ring-brand",
								}
								span {
									class: "text-sm text-content-secondary",
									"Remember me"
								}
							}
							a {
								href: "#",
								class: "text-sm text-brand hover:text-brand-hover",
								"Forgot password?"
							}
						}
						div {
							class: "mt-5",
							watch {
								if loading_signal.get() {
									button {
										r#type: "submit",
										class: "btn-primary w-full opacity-50 cursor-not-allowed",
										disabled: loading_signal.get(),
										form: "login-form",
										div {
											class: "flex items-center justify-center gap-2",
											div {
												class: "spinner-sm border-white border-t-transparent",
											}
											"Signing in..."
										}
									}
								} else {
									button {
										r#type: "submit",
										class: "btn-primary w-full",
										form: "login-form",
										"Sign in"
									}
								}
							}
						}
					}
				}
				div {
					class: "text-center mt-6",
					span {
						class: "text-content-secondary",
						"Don't have an account? "
					}
					a {
						href: "/register",
						class: "text-brand font-semibold hover:text-brand-hover",
						"Sign up"
					}
				}
			}
		}
	})(loading_signal, error_signal, form_view)
}

/// Registration form component using form! macro
///
/// Uses the `form!` macro for:
/// - Declarative field definitions (username, email, password, password_confirmation)
/// - UI state management via `state: { loading, error }`
/// - Two-way binding via `bind: true` (default)
/// - SVG icons via `icon` property
/// - Server function integration via `server_fn`
/// - `redirect_on_success` for navigation after registration
pub fn register_form() -> View {
	// Define form with state management and field definitions
	let register_form = form! {
		name: RegisterForm,
		server_fn: register,

		// UI state management
		state: { loading, error },

		// Redirect after successful registration
		redirect_on_success: "/login",

		fields: {
			username: CharField {
				label: "Username",
				max_length: 150,
				placeholder: "Choose a username",
				wrapper: div { class: "relative" },
				icon: svg {
					class: "w-5 h-5 text-content-tertiary",
					fill: "none",
					stroke: "currentColor",
					viewBox: "0 0 24 24",
					path {
						stroke_linecap: "round",
						stroke_linejoin: "round",
						stroke_width: "2",
						d: "M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"
					}
				},
				icon_position: "left",
				class: "form-input pl-10",
			},
			email: EmailField {
				label: "Email",
				placeholder: "you@example.com",
				wrapper: div { class: "relative" },
				icon: svg {
					class: "w-5 h-5 text-content-tertiary",
					fill: "none",
					stroke: "currentColor",
					viewBox: "0 0 24 24",
					path {
						stroke_linecap: "round",
						stroke_linejoin: "round",
						stroke_width: "2",
						d: "M16 12a4 4 0 10-8 0 4 4 0 008 0zm0 0v1.5a2.5 2.5 0 005 0V12a9 9 0 10-9 9m4.5-1.206a8.959 8.959 0 01-4.5 1.207"
					}
				},
				icon_position: "left",
				class: "form-input pl-10",
			},
			password: PasswordField {
				label: "Password",
				placeholder: "Choose a password",
				wrapper: div { class: "relative" },
				icon: svg {
					class: "w-5 h-5 text-content-tertiary",
					fill: "none",
					stroke: "currentColor",
					viewBox: "0 0 24 24",
					path {
						stroke_linecap: "round",
						stroke_linejoin: "round",
						stroke_width: "2",
						d: "M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"
					}
				},
				icon_position: "left",
				class: "form-input pl-10",
			},
			password_confirmation: PasswordField {
				label: "Confirm Password",
				placeholder: "Confirm your password",
				wrapper: div { class: "relative" },
				icon: svg {
					class: "w-5 h-5 text-content-tertiary",
					fill: "none",
					stroke: "currentColor",
					viewBox: "0 0 24 24",
					path {
						stroke_linecap: "round",
						stroke_linejoin: "round",
						stroke_width: "2",
						d: "M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z"
					}
				},
				icon_position: "left",
				class: "form-input pl-10",
			},
		},
	};

	// Clone state signals for page! macro
	let loading_signal = register_form.loading().clone();
	let error_signal = register_form.error().clone();

	// Convert form! to View before passing to page!
	let form_view = register_form.into_view();

	// Render custom UI using page! macro
	page!(|loading_signal: Signal<bool>, error_signal: Signal<Option<String>>, form_view: View| {
		div {
			class: "min-h-screen flex items-center justify-center px-4 py-12 bg-surface-secondary",
			div {
				class: "w-full max-w-md",
				div {
					class: "text-center mb-8",
					div {
						class: "inline-flex items-center justify-center w-16 h-16 rounded-full bg-brand/10 mb-4",
						svg {
							class: "w-8 h-8 text-brand",
							fill: "none",
							stroke: "currentColor",
							viewBox: "0 0 24 24",
							path {
								stroke_linecap: "round",
								stroke_linejoin: "round",
								stroke_width: "2",
								d: "M18 9v3m0 0v3m0-3h3m-3 0h-3m-2-5a4 4 0 11-8 0 4 4 0 018 0zM3 20a6 6 0 0112 0v1H3v-1z",
							}
						}
					}
					h1 {
						class: "text-2xl font-bold text-content-primary",
						"Create an account"
					}
					p {
						class: "text-content-secondary mt-2",
						"Join the conversation today"
					}
				}
				div {
					class: "card animate-fade-in",
					div {
						class: "card-body p-6 sm:p-8",
						watch {
							if error_signal.get().is_some() {
								div {
									class: "alert-danger mb-4",
									div {
										class: "flex items-center gap-2",
										svg {
											class: "w-5 h-5 flex-shrink-0",
											fill: "currentColor",
											viewBox: "0 0 20 20",
											path {
												fill_rule: "evenodd",
												d: "M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z",
											}
										}
										span {
											{ error_signal.get().unwrap_or_default() }
										}
									}
								}
							}
						}
						{ form_view }
						div {
							class: "flex items-start gap-2 mt-4",
							input {
								r#type: "checkbox",
								class: "w-4 h-4 mt-1 rounded border-border text-brand focus:ring-brand",
								id: "terms",
							}
							label {
								r#for: "terms",
								class: "text-sm text-content-secondary",
								"I agree to the "
								span {
									class: "text-brand hover:text-brand-hover cursor-pointer",
									"Terms of Service"
								}
								" and "
								span {
									class: "text-brand hover:text-brand-hover cursor-pointer",
									"Privacy Policy"
								}
							}
						}
						div {
							class: "mt-5",
							watch {
								if loading_signal.get() {
									button {
										r#type: "submit",
										class: "btn-primary w-full opacity-50 cursor-not-allowed",
										disabled: loading_signal.get(),
										form: "register-form",
										div {
											class: "flex items-center justify-center gap-2",
											div {
												class: "spinner-sm border-white border-t-transparent",
											}
											"Creating account..."
										}
									}
								} else {
									button {
										r#type: "submit",
										class: "btn-primary w-full",
										form: "register-form",
										"Create account"
									}
								}
							}
						}
					}
				}
				div {
					class: "text-center mt-6",
					span {
						class: "text-content-secondary",
						"Already have an account? "
					}
					a {
						href: "/login",
						class: "text-brand font-semibold hover:text-brand-hover",
						"Sign in"
					}
				}
			}
		}
	})(loading_signal, error_signal, form_view)
}
