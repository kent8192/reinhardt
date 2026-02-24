//! Relationship components (follow/block)
//!
//! Provides follow button and user list components for managing user relationships.

use crate::apps::auth::shared::types::UserInfo;
use reinhardt::pages::Signal;
use reinhardt::pages::component::View;
use reinhardt::pages::page;
use uuid::Uuid;

#[cfg(client)]
use {
	crate::apps::relationship::server::server_fn::{
		fetch_followers, fetch_following, follow_user, unfollow_user,
	},
	reinhardt::pages::spawn::spawn_task,
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
pub fn follow_button(target_user_id: Uuid, is_following_initial: bool) -> View {
	let is_following = Signal::new(is_following_initial);
	let loading = Signal::new(false);
	let error = Signal::new(None::<String>);

	// Clone signals for passing to page! macro
	let is_following_signal = is_following.clone();
	let loading_signal = loading.clone();
	let error_signal = error.clone();

	#[cfg(client)]
	{
		let is_following_clone = is_following.clone();
		let loading_clone = loading.clone();
		let error_clone = error.clone();

		page!(|is_following_signal: Signal<bool>, loading_signal: Signal<bool>, error_signal: Signal<Option<String>>| {
			div {
				watch {
					if loading_signal.get() {
						button {
							r#type: "button",
							class: "btn-secondary opacity-50 cursor-not-allowed",
							disabled: loading_signal.get(),
							aria_label: "Loading",
							@click: {
										let is_following = is_following_clone.clone();
										let loading = loading_clone.clone();
										let error = error_clone.clone();
										move |_event| {
											let is_following_inner = is_following.clone();
											let loading_inner = loading.clone();
											let error_inner = error.clone();
											let currently_following = is_following.get();
											spawn_task(async move {
												loading_inner.set(true);
												error_inner.set(None);
												let result = if currently_following {
													unfollow_user(target_user_id).await
												} else {
													follow_user(target_user_id).await
												};
												match result {
													Ok(()) => {
														is_following_inner.set(!currently_following);
														loading_inner.set(false);
													}
													Err(e) => {
														error_inner.set(Some(e.to_string()));
														loading_inner.set(false);
													}
												}
											});
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
							r#type: "button",
							class: "btn-outline group",
							@click: {
										let is_following = is_following_clone.clone();
										let loading = loading_clone.clone();
										let error = error_clone.clone();
										move |_event| {
											let is_following_inner = is_following.clone();
											let loading_inner = loading.clone();
											let error_inner = error.clone();
											let currently_following = is_following.get();
											spawn_task(async move {
												loading_inner.set(true);
												error_inner.set(None);
												let result = if currently_following {
													unfollow_user(target_user_id).await
												} else {
													follow_user(target_user_id).await
												};
												match result {
													Ok(()) => {
														is_following_inner.set(!currently_following);
														loading_inner.set(false);
													}
													Err(e) => {
														error_inner.set(Some(e.to_string()));
														loading_inner.set(false);
													}
												}
											});
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
							r#type: "button",
							class: "btn-primary",
							@click: {
										let is_following = is_following_clone.clone();
										let loading = loading_clone.clone();
										let error = error_clone.clone();
										move |_event| {
											let is_following_inner = is_following.clone();
											let loading_inner = loading.clone();
											let error_inner = error.clone();
											let currently_following = is_following.get();
											spawn_task(async move {
												loading_inner.set(true);
												error_inner.set(None);
												let result = if currently_following {
													unfollow_user(target_user_id).await
												} else {
													follow_user(target_user_id).await
												};
												match result {
													Ok(()) => {
														is_following_inner.set(!currently_following);
														loading_inner.set(false);
													}
													Err(e) => {
														error_inner.set(Some(e.to_string()));
														loading_inner.set(false);
													}
												}
											});
										}
									},
							"Follow"
						}
					}
				}
				watch {
					if error_signal.get().is_some() {
						div {
							class: "alert-danger mt-2 text-sm",
							{ error_signal.get().unwrap_or_default() }
						}
					}
				}
			}
		})(is_following_signal, loading_signal, error_signal)
	}

	#[cfg(server)]
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
					r#type: "button",
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
fn user_card(user: &UserInfo) -> View {
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
				svg {
					class: "w-5 h-5 text-content-tertiary flex-shrink-0",
					fill: "none",
					stroke: "currentColor",
					viewBox: "0 0 24 24",
					path {
						stroke_linecap: "round",
						stroke_linejoin: "round",
						stroke_width: "2",
						d: "M9 5l7 7-7 7",
					}
				}
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
pub fn user_list(user_id: Uuid, list_type: UserListType) -> View {
	let users = Signal::new(Vec::<UserInfo>::new());
	let loading = Signal::new(true);
	let error = Signal::new(None::<String>);

	#[cfg(client)]
	{
		let users_clone = users.clone();
		let loading_clone = loading.clone();
		let error_clone = error.clone();

		spawn_task(async move {
			loading_clone.set(true);
			error_clone.set(None);

			let result = match list_type {
				UserListType::Followers => fetch_followers(user_id).await,
				UserListType::Following => fetch_following(user_id).await,
			};

			match result {
				Ok(user_list) => {
					users_clone.set(user_list);
					loading_clone.set(false);
				}
				Err(e) => {
					error_clone.set(Some(e.to_string()));
					loading_clone.set(false);
				}
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
						{ View::fragment(users_signal.get().iter().map(|u| user_card(u)).collect ::<Vec<_>>()) }
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
