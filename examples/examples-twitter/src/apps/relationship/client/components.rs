//! Relationship components (follow/block)
//!
//! Provides follow button and user list components for managing user relationships.

use crate::apps::auth::shared::types::UserInfo;
use crate::core::client::components::icons;
use reinhardt::pages::Signal;
use reinhardt::pages::component::Page;
use reinhardt::pages::page;
use uuid::Uuid;

#[cfg(wasm)]
use {
	crate::apps::relationship::shared::server_fn::{
		fetch_followers, fetch_following, follow_user, unfollow_user,
	},
	reinhardt::pages::create_resource,
	reinhardt::pages::reactive::ResourceState,
	reinhardt::pages::reactive::hooks::{Action, use_action, use_effect},
};

/// Type of user list to display
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UserListType {
	/// List of followers
	Followers,
	/// List of users being followed
	Following,
}

/// Follow button component
///
/// Provides a button to follow/unfollow a user with state management.
/// Modern design with visual feedback for following state.
/// Uses watch blocks for reactive UI updates when state changes.
pub fn follow_button(target_user_id: Uuid, is_following_initial: bool) -> Page {
	let is_following = Signal::new(is_following_initial);

	// Clone signal for passing to page! macro
	let is_following_signal = is_following.clone();

	#[cfg(wasm)]
	{
		let toggle_follow = use_action(
			move |(target_id, currently_following): (Uuid, bool)| async move {
				if currently_following {
					unfollow_user(target_id).await
				} else {
					follow_user(target_id).await
				}
				.map_err(|e| e.to_string())
			},
		);

		// Toggle is_following on success and reset the action
		{
			let toggle_follow_for_effect = toggle_follow.clone();
			let is_following_for_effect = is_following.clone();
			use_effect(move || {
				if toggle_follow_for_effect.is_success() {
					let current = is_following_for_effect.get();
					is_following_for_effect.set(!current);
					toggle_follow_for_effect.reset();
				}
			});
		}

		let toggle_follow_for_error = toggle_follow.clone();

		page!(|is_following_signal: Signal<bool>, toggle_follow: Action<(), String>, toggle_follow_for_error: Action<(), String>| {
			div {
				watch {
					if toggle_follow.is_pending() {
						button {
							type: "button",
							class: "btn-secondary opacity-50 cursor-not-allowed",
							disabled: { true },
							aria_label: "Loading",
							@click: {
										let toggle_follow = toggle_follow.clone();
										let is_following_signal = is_following_signal.clone();
										move |_event| {
											toggle_follow.dispatch((target_user_id, is_following_signal.get()));
										}
									},
							div {
								class: "flex items-center gap-2",
								div {
									class: "spinner-sm",
								}
							}
						}
					} else if is_following_signal.get() {
						button {
							type: "button",
							class: "btn-outline group",
							@click: {
										let toggle_follow = toggle_follow.clone();
										let is_following_signal = is_following_signal.clone();
										move |_event| {
											toggle_follow.dispatch((target_user_id, is_following_signal.get()));
										}
									},
							span {
								class: "group-hover:hidden",
								"Following"
							}
							span {
								class: "hidden group-hover:inline text-danger",
								"Unfollow"
							}
						}
					} else {
						button {
							type: "button",
							class: "btn-primary",
							@click: {
										let toggle_follow = toggle_follow.clone();
										let is_following_signal = is_following_signal.clone();
										move |_event| {
											toggle_follow.dispatch((target_user_id, is_following_signal.get()));
										}
									},
							"Follow"
						}
					}
				}
				watch {
					if toggle_follow_for_error.error().is_some() {
						div {
							class: "alert-danger mt-2 text-sm",
							{ toggle_follow_for_error.error().unwrap_or_default() }
						}
					}
				}
			}
		})(
			is_following_signal,
			toggle_follow,
			toggle_follow_for_error,
		)
	}

	#[cfg(native)]
	{
		// For SSR, render initial state without event handlers
		let btn_class = if is_following_initial {
			"btn-outline group"
		} else {
			"btn-primary"
		};
		let btn_text = if is_following_initial {
			"Following"
		} else {
			"Follow"
		};

		page!(|btn_class: &str, btn_text: &str| {
			div {
				button {
					type: "button",
					class: btn_class,
					{ btn_text }
				}
			}
		})(btn_class, btn_text)
	}
}

/// User card component
///
/// Displays a single user in a list with modern SNS design.
/// Features avatar, username, and profile link.
fn user_card(user: &UserInfo) -> Page {
	let username = user.username.clone();
	let display_username = format!("@{}", user.username);
	let email = user.email.clone();
	let profile_url = format!("/profile/{}", user.id);
	let avatar_initial = user
		.username
		.chars()
		.next()
		.unwrap_or('U')
		.to_uppercase()
		.to_string();

	page!(|username: String, display_username: String, _email: String, profile_url: String, avatar_initial: String| {
		a {
			href: profile_url.clone(),
			class: "user-card block",
			div {
				class: "flex items-center gap-3",
				div {
					class: "user-avatar bg-surface-tertiary flex items-center justify-center text-content-secondary font-semibold flex-shrink-0",
					{ avatar_initial }
				}
				div {
					class: "flex-1 min-w-0",
					div {
						class: "font-semibold text-content-primary truncate",
						{ username }
					}
					div {
						class: "text-content-secondary text-sm truncate",
						{ display_username }
					}
				}
				{ icons::chevron_right_icon() }
			}
		}
	})(
		username,
		display_username,
		email,
		profile_url,
		avatar_initial,
	)
}

/// User list component
///
/// Displays a list of users (followers or following) with loading and error states.
/// Modern card-based design with smooth animations.
/// Uses watch blocks for reactive UI updates when async data loads.
pub fn user_list(user_id: Uuid, list_type: UserListType) -> Page {
	let users = Signal::new(Vec::<UserInfo>::new());
	let loading = Signal::new(true);
	let error = Signal::new(None::<String>);

	#[cfg(wasm)]
	{
		let resource = create_resource(move || async move {
			let result = match list_type {
				UserListType::Followers => fetch_followers(user_id).await,
				UserListType::Following => fetch_following(user_id).await,
			};
			result.map_err(|e| e.to_string())
		});

		let users_clone = users.clone();
		let loading_clone = loading.clone();
		let error_clone = error.clone();
		let resource_for_effect = resource.clone();

		use_effect(move || match resource_for_effect.get() {
			ResourceState::Loading => {
				loading_clone.set(true);
				error_clone.set(None);
			}
			ResourceState::Success(data) => {
				users_clone.set(data);
				loading_clone.set(false);
				error_clone.set(None);
			}
			ResourceState::Error(err) => {
				error_clone.set(Some(err));
				loading_clone.set(false);
			}
		});
	}

	let title = match list_type {
		UserListType::Followers => "Followers",
		UserListType::Following => "Following",
	}
	.to_string();

	let empty_message = match list_type {
		UserListType::Followers => "No followers yet",
		UserListType::Following => "Not following anyone yet",
	}
	.to_string();

	let empty_icon = match list_type {
		UserListType::Followers => "M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z",
		UserListType::Following => "M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z",
	}
	.to_string();

	// Clone signals for passing to page! macro
	let users_signal = users.clone();
	let loading_signal = loading.clone();
	let error_signal = error.clone();

	page!(|title: String, empty_message: String, empty_icon: String, users_signal: Signal<Vec<UserInfo>>, loading_signal: Signal<bool>, error_signal: Signal<Option<String>>| {
		div {
			class: "animate-fade-in",
			div {
				class: "flex items-center gap-3 mb-4",
				a {
					href: "/",
					class: "btn-icon",
					aria_label: "Go back home",
					{ icons::arrow_left_icon() }
				}
				h2 {
					class: "text-xl font-bold text-content-primary",
					{ title }
				}
			}
			watch {
				if loading_signal.get() {
					div {
						class: "flex flex-col items-center justify-center py-12",
						div {
							class: "spinner-lg mb-4",
						}
						p {
							class: "text-content-secondary text-sm",
							"Loading..."
						}
					}
				} else if error_signal.get().is_some() {
					div {
						class: "alert-danger",
						div {
							class: "flex items-center gap-2",
							{ icons::error_circle_icon() }
							span {
								{ error_signal.get().unwrap_or_default() }
							}
						}
					}
				} else if users_signal.get().is_empty() {
					div {
						class: "flex flex-col items-center justify-center py-16 text-center",
						div {
							class: "w-16 h-16 rounded-full bg-surface-tertiary flex items-center justify-center mb-4",
							svg {
								class: "w-8 h-8 text-content-tertiary",
								fill: "none",
								stroke: "currentColor",
								viewBox: "0 0 24 24",
								path {
									stroke_linecap: "round",
									stroke_linejoin: "round",
									stroke_width: "1.5",
									d: empty_icon.clone(),
								}
							}
						}
						p {
							class: "text-content-secondary",
							{ empty_message.clone() }
						}
					}
				} else {
					div {
						class: "card overflow-hidden",
						{
							Page::Fragment(
									users_signal
										.get()
										.iter()
										.map(|u| user_card(u))
										.collect::<Vec<_>>(),
								)
						}
					}
				}
			}
		}
	})(
		title,
		empty_message,
		empty_icon,
		users_signal,
		loading_signal,
		error_signal,
	)
}
