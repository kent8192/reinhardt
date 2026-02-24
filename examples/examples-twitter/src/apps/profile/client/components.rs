//! Profile components using React-like hooks
//!
//! Provides profile view and edit form components with hooks-styled state management.
//! Validation is handled server-side via server functions with automatic CSRF protection.
//!
//! The `profile_edit` component uses the `form!` macro for:
//! - Declarative field definitions with reactive Signal management
//! - UI state management (loading, error, success)
//! - Two-way binding (bind: true by default)
//! - SVG icons with custom positioning
//! - Server function integration for form submission

use crate::apps::profile::shared::types::ProfileResponse;
use reinhardt::pages::component::View;
use reinhardt::pages::form;
use reinhardt::pages::page;
use reinhardt::pages::reactive::Signal;
use reinhardt::pages::reactive::hooks::use_state;
use uuid::Uuid;

#[cfg(client)]
use {
	crate::apps::profile::server::server_fn::{fetch_profile, update_profile_form},
	reinhardt::pages::spawn::spawn_task,
};

#[cfg(server)]
use crate::apps::profile::server::server_fn::fetch_profile;

/// Profile view component using hooks
///
/// Displays user profile information with modern SNS design.
/// Features cover image, large avatar, bio, location, and website.
/// Uses watch blocks for reactive UI updates when async data loads.
pub fn profile_view(user_id: Uuid) -> View {
	// Hook-styled state management
	let (profile, set_profile) = use_state(None::<ProfileResponse>);
	let (loading, set_loading) = use_state(true);
	let (error, set_error) = use_state(None::<String>);

	#[cfg(client)]
	{
		// Clone setters for async use
		let set_profile = set_profile.clone();
		let set_loading = set_loading.clone();
		let set_error = set_error.clone();

		spawn_task(async move {
			set_loading(true);
			set_error(None);

			match fetch_profile(user_id).await {
				Ok(profile_data) => {
					set_profile(Some(profile_data));
					set_loading(false);
				}
				Err(e) => {
					set_error(Some(e.to_string()));
					set_loading(false);
				}
			}
		});
	}

	// Clone signals for passing to page! macro
	let loading_signal = loading.clone();
	let error_signal = error.clone();
	let profile_signal = profile.clone();

	// Clone user_id for use in page! macro
	let user_id_str = user_id.to_string();

	page!(|loading_signal: Signal<bool>, error_signal: Signal<Option<String>>, profile_signal: Signal<Option<ProfileResponse>>, user_id_str: String| {
		div {
			class: "max-w-2xl mx-auto",
			watch {
				if loading_signal.get() {
					div {
						class: "flex flex-col items-center justify-center py-16",
						div {
							class: "spinner-lg mb-4",
						}
						p {
							class: "text-content-secondary text-sm",
							"Loading profile..."
						}
					}
				} else if error_signal.get().is_some() {
					div {
						class: "p-4",
						div {
							class: "alert-danger",
							role: "alert",
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
				} else if profile_signal.get().is_some() {
					div {
						class: "card overflow-hidden animate-fade-in",
						div {
							class: "h-32 sm:h-48 bg-gradient-to-r from-brand to-brand-dark relative",
						}
						div {
							class: "px-4 pb-4",
							div {
								class: "flex justify-between items-end -mt-12 sm:-mt-16 mb-4",
								div {
									class: "avatar-xl sm:w-32 sm:h-32 rounded-full border-4 border-surface-primary bg-surface-tertiary flex items-center justify-center text-3xl sm:text-4xl font-bold text-content-secondary",
									span {
										"👤"
									}
								}
								a {
									href: format!("/profile/{}/edit", user_id_str),
									class: "btn-outline",
									"Edit profile"
								}
							}
							div {
								class: "mb-4",
								h1 {
									class: "text-xl font-bold text-content-primary",
									"@user"
								}
							}
							if let Some(ref data) = profile_signal.get() {
								if data.bio.is_some() {
									p {
										class: "text-content-primary mb-4 whitespace-pre-wrap",
										{ data.bio.clone().unwrap_or_default() }
									}
								}
							}
							div {
								class: "flex flex-wrap gap-4 text-content-secondary text-sm",
								if let Some(ref data) = profile_signal.get() {
									if data.location.is_some() {
										div {
											class: "flex items-center gap-1",
											svg {
												class: "w-4 h-4",
												fill: "none",
												stroke: "currentColor",
												viewBox: "0 0 24 24",
												path {
													stroke_linecap: "round",
													stroke_linejoin: "round",
													stroke_width: "2",
													d: "M17.657 16.657L13.414 20.9a1.998 1.998 0 01-2.827 0l-4.244-4.243a8 8 0 1111.314 0z",
												}
												path {
													stroke_linecap: "round",
													stroke_linejoin: "round",
													stroke_width: "2",
													d: "M15 11a3 3 0 11-6 0 3 3 0 016 0z",
												}
											}
											span {
												{ data.location.clone().unwrap_or_default() }
											}
										}
									}
									if data.website.is_some() {
										a {
											class: "flex items-center gap-1 text-brand hover:underline",
											href: data.website.clone().unwrap_or_default(),
											target: "_blank",
											rel: "noopener noreferrer",
											svg {
												class: "w-4 h-4",
												fill: "none",
												stroke: "currentColor",
												viewBox: "0 0 24 24",
												path {
													stroke_linecap: "round",
													stroke_linejoin: "round",
													stroke_width: "2",
													d: "M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1",
												}
											}
											span {
												{ data.website.clone().unwrap_or_default() }
											}
										}
									}
								}
							}
							div {
								class: "flex gap-6 mt-4 pt-4 border-t border-border",
								div {
									class: "flex items-center gap-1",
									span {
										class: "font-bold text-content-primary",
										"0"
									}
									span {
										class: "text-content-secondary text-sm",
										"Following"
									}
								}
								div {
									class: "flex items-center gap-1",
									span {
										class: "font-bold text-content-primary",
										"0"
									}
									span {
										class: "text-content-secondary text-sm",
										"Followers"
									}
								}
							}
						}
					}
				}
			}
		}
	})(loading_signal, error_signal, profile_signal, user_id_str)
}

/// Profile edit component using form! macro with state management
///
/// Uses the `form!` macro for:
/// - Declarative field definitions (avatar_url, bio, location, website)
/// - UI state management via `state: { loading, error, success }`
/// - Two-way binding via `bind: true` (default)
/// - SVG icons via `icon` property
/// - Server function integration via `server_fn`
///
/// The form uses custom UnoCSS styling and card layout through page! macro,
/// while form! handles all Signal management and form submission logic.
pub fn profile_edit(user_id: Uuid) -> View {
	// Define form with state management and field definitions
	// form! macro generates:
	// - Signal<String> for each field with automatic two-way binding
	// - Signal<bool> for loading state
	// - Signal<Option<String>> for error state
	// - Signal<bool> for success state
	// - Accessor methods: avatar_url(), bio(), location(), website()
	// - State accessors: loading(), error(), success()
	// - submit() method that calls server_fn
	let profile_form = form! {
		name: ProfileEditForm,
		server_fn: update_profile_form,

		// UI state management - replaces manual use_state calls
		state: { loading, error, success },

		fields: {
			avatar_url: UrlField {
				label: "Avatar URL",
				placeholder: "https://example.com/avatar.jpg",
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
						d: "M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z"
					}
				},
				icon_position: "left",
				class: "form-input pl-10",
			},
			bio: TextField {
				label: "Bio",
				max_length: 500,
				placeholder: "Tell the world about yourself...",
				class: "form-textarea",
			},
			location: CharField {
				label: "Location",
				max_length: 100,
				placeholder: "San Francisco, CA",
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
						d: "M17.657 16.657L13.414 20.9a1.998 1.998 0 01-2.827 0l-4.244-4.243a8 8 0 1111.314 0z"
					}
					path {
						stroke_linecap: "round",
						stroke_linejoin: "round",
						stroke_width: "2",
						d: "M15 11a3 3 0 11-6 0 3 3 0 016 0z"
					}
				},
				icon_position: "left",
				class: "form-input pl-10",
			},
			website: UrlField {
				label: "Website",
				placeholder: "https://example.com",
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
						d: "M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1"
					}
				},
				icon_position: "left",
				class: "form-input pl-10",
			},
		},
	};

	// Load current profile data into form fields
	// Note: initial_loader doesn't support parameters, so we load manually
	#[cfg(client)]
	{
		let avatar_url_signal = profile_form.avatar_url().clone();
		let bio_signal = profile_form.bio().clone();
		let location_signal = profile_form.location().clone();
		let website_signal = profile_form.website().clone();

		spawn_task(async move {
			if let Ok(profile_data) = fetch_profile(user_id).await {
				avatar_url_signal.set(profile_data.avatar_url.unwrap_or_default());
				bio_signal.set(profile_data.bio.unwrap_or_default());
				location_signal.set(profile_data.location.unwrap_or_default());
				website_signal.set(profile_data.website.unwrap_or_default());
			}
		});
	}

	// Clone state signals for page! macro
	let loading_signal = profile_form.loading().clone();
	let error_signal = profile_form.error().clone();
	let success_signal = profile_form.success().clone();

	// Convert form! to View before passing to page!
	// into_view() consumes self, so we call it after cloning signals
	let form_view = profile_form.into_view();

	let user_id_str = user_id.to_string();

	// Render custom UI using page! macro
	// form! handles Signal management, page! handles custom layout
	page!(|loading_signal: Signal<bool>, error_signal: Signal<Option<String>>, success_signal: Signal<bool>, form_view: View, user_id_str: String| {
		div {
			class: "max-w-2xl mx-auto p-4",
			div {
				class: "card animate-fade-in",
				div {
					class: "card-header flex items-center gap-3",
					a {
						href: format!("/profile/{}", user_id_str.clone()),
						class: "btn-icon",
						svg {
							class: "w-5 h-5",
							fill: "none",
							stroke: "currentColor",
							viewBox: "0 0 24 24",
							path {
								stroke_linecap: "round",
								stroke_linejoin: "round",
								stroke_width: "2",
								d: "M10 19l-7-7m0 0l7-7m-7 7h18",
							}
						}
					}
					h1 {
						class: "text-xl font-bold",
						"Edit Profile"
					}
				}
				div {
					class: "card-body",
					watch {
						if success_signal.get() {
							div {
								class: "alert-success mb-4",
								role: "alert",
								div {
									class: "flex items-center gap-2",
									svg {
										class: "w-5 h-5 flex-shrink-0",
										fill: "currentColor",
										viewBox: "0 0 20 20",
										path {
											fill_rule: "evenodd",
											d: "M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z",
										}
									}
									span {
										"Profile updated successfully! Redirecting..."
									}
								}
							}
						}
					}
					watch {
						if error_signal.get().is_some() {
							div {
								class: "alert-danger mb-4",
								role: "alert",
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
						class: "flex justify-end gap-3 pt-4 border-t border-border mt-5",
						a {
							href: format!("/profile/{}", user_id_str),
							class: "btn-secondary",
							"Cancel"
						}
						watch {
							if loading_signal.get() {
								button {
									r#type: "submit",
									class: "btn-primary opacity-50 cursor-not-allowed",
									disabled: loading_signal.get(),
									form: "profile-edit-form",
									div {
										class: "flex items-center gap-2",
										div {
											class: "spinner-sm border-white border-t-transparent",
										}
										"Saving..."
									}
								}
							} else {
								button {
									r#type: "submit",
									class: "btn-primary",
									form: "profile-edit-form",
									"Save"
								}
							}
						}
					}
				}
			}
		}
	})(
		loading_signal,
		error_signal,
		success_signal,
		form_view,
		user_id_str,
	)
}
