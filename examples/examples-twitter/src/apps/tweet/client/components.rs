//! Tweet components using React-like hooks and form! macro
//!
//! Provides tweet card, tweet form, and tweet list components.
//! tweet_form uses the form! macro with derived blocks for computed signals,
//! while tweet_card and tweet_list use page! macro with hooks-styled state management.

use crate::apps::tweet::shared::types::TweetInfo;
use crate::core::client::components::icons;
use reinhardt::pages::Signal;
use reinhardt::pages::component::Page;
use reinhardt::pages::form;
use reinhardt::pages::page;
use reinhardt::pages::reactive::hooks::{Action, use_action, use_effect, use_state};
use uuid::Uuid;

#[cfg(wasm)]
use {
	crate::apps::tweet::shared::server_fn::{create_tweet, delete_tweet, list_tweets},
	reinhardt::pages::create_resource,
	reinhardt::pages::reactive::ResourceState,
};

#[cfg(native)]
use crate::apps::tweet::shared::server_fn::{create_tweet, delete_tweet};

/// Like button component (extracted to avoid nested watch blocks)
///
/// This function is separated from tweet_card to avoid nested watch block issues
/// with closure ownership in the page! macro.
fn like_button(liked: Signal<bool>, like_count: Signal<i32>) -> Page {
	// Clone signals for the watch block
	let liked_signal = liked.clone();
	let like_count_signal = like_count.clone();
	let like_count_signal_else = like_count.clone();
	// Clone signals for event handlers
	let liked_for_click_if = liked.clone();
	let like_count_for_click_if = like_count.clone();
	let liked_for_click_else = liked.clone();
	let like_count_for_click_else = like_count.clone();

	page!(|liked_signal: Signal<bool>, like_count_signal: Signal<i32>, like_count_signal_else: Signal<i32>, liked_for_click_if: Signal<bool>, like_count_for_click_if: Signal<i32>, liked_for_click_else: Signal<bool>, like_count_for_click_else: Signal<i32>| {
		watch {
			if liked_signal.get() {
				button {
					class: "tweet-action-btn text-danger",
					type: "button",
					aria_label: "Like",
					@click: {
								let liked_for_click = liked_for_click_if.clone();
								let like_count_for_click = like_count_for_click_if.clone();
								move |_event| {
									let current_liked = liked_for_click.get();
									let current_count = like_count_for_click.get();
									liked_for_click.set(!current_liked);
									like_count_for_click.set(if current_liked {
										current_count - 1
									} else {
										current_count + 1
									});
								}
							},
					{ icons::heart_icon_filled() }
					span {
						{ format!("{}", like_count_signal.get()) }
					}
				}
			} else {
				button {
					class: "tweet-action-btn hover:text-danger",
					type: "button",
					aria_label: "Like",
					@click: {
								let liked_for_click = liked_for_click_else.clone();
								let like_count_for_click = like_count_for_click_else.clone();
								move |_event| {
									let current_liked = liked_for_click.get();
									let current_count = like_count_for_click.get();
									liked_for_click.set(!current_liked);
									like_count_for_click.set(if current_liked {
										current_count - 1
									} else {
										current_count + 1
									});
								}
							},
					{ icons::heart_icon_outline() }
					span {
						{ format!("{}", like_count_signal_else.get()) }
					}
				}
			}
		}
	})(
		liked_signal,
		like_count_signal,
		like_count_signal_else,
		liked_for_click_if,
		like_count_for_click_if,
		liked_for_click_else,
		like_count_for_click_else,
	)
}

/// Tweet card component using hooks
///
/// Displays a single tweet with modern SNS design (Threads/Bluesky-inspired).
/// Features avatar, username, handle, content, timestamp, and action buttons.
/// Uses watch blocks for reactive UI updates when state changes.
pub fn tweet_card(tweet: &TweetInfo, show_delete: bool) -> Page {
	let tweet_id = tweet.id;

	// Hook-styled state management
	let delete_action =
		use_action(
			move |tid: Uuid| async move { delete_tweet(tid).await.map_err(|e| e.to_string()) },
		);
	let (liked, _set_liked) = use_state(false);
	let (like_count, _set_like_count) = use_state(0i32);

	// Clone liked/like_count signals so we can call like_button inside watch
	let liked_signal = liked.clone();
	let like_count_signal = like_count.clone();

	// Clone tweet data for use in page! macro
	let username = tweet.username.clone();
	let content = tweet.content.clone();
	let created_at = tweet.created_at.clone();

	// Clone delete_action for the click handler closure
	let delete_action_for_click = delete_action.clone();

	// Clone for error display watch block (separate closure from main watch block)
	let delete_action_for_error = delete_action.clone();

	page!(|delete_action: Action<(), String>, show_delete: bool, username: String, content: String, created_at: String, tweet_id: Uuid, liked_signal: Signal<bool>, like_count_signal: Signal<i32>, delete_action_for_click: Action<(), String>, delete_action_for_error: Action<(), String>| {
		watch {
			if delete_action.is_success() {
				div {
					class: "hidden",
				}
			} else {
				div {
					class: "tweet-card animate-fade-in",
					div {
						class: "flex gap-3",
						div {
							class: "flex-shrink-0",
							div {
								class: "tweet-avatar bg-surface-tertiary flex items-center justify-center text-content-secondary font-semibold",
								{
									username
											.clone()
											.chars()
											.next()
											.unwrap_or('U')
											.to_uppercase()
											.to_string()
								}
							}
						}
						div {
							class: "flex-1 min-w-0",
							div {
								class: "flex items-center justify-between gap-2",
								div {
									class: "flex items-center gap-1 min-w-0",
									span {
										class: "tweet-username truncate",
										{ username.clone() }
									}
									span {
										class: "tweet-handle truncate",
										{ format!("@{}", username.clone()) }
									}
									span {
										class: "text-content-tertiary",
										"·"
									}
									span {
										class: "tweet-time",
										{ created_at.clone() }
									}
								}
								if show_delete {
									button {
										class: "btn-ghost btn-sm text-danger hover:bg-danger/10",
										type: "button",
										aria_label: "Delete tweet",
										@click: {
													let delete_action = delete_action_for_click.clone();
													move |_event| {
														delete_action.dispatch(tweet_id);
													}
												},
										{ icons::trash_icon() }
									}
								}
							}
							p {
								class: "tweet-content",
								{ content.clone() }
							}
							div {
								class: "tweet-actions",
								button {
									class: "tweet-action-btn hover:text-brand",
									type: "button",
									aria_label: "Reply",
									{ icons::chat_bubble_icon() }
									span {
										"0"
									}
								}
								button {
									class: "tweet-action-btn hover:text-success",
									type: "button",
									aria_label: "Retweet",
									{ icons::retweet_icon() }
									span {
										"0"
									}
								}
								{ like_button(liked_signal.clone(), like_count_signal.clone()) }
								button {
									class: "tweet-action-btn hover:text-brand",
									type: "button",
									aria_label: "Share",
									{ icons::share_icon() }
								}
							}
						}
					}
				}
			}
		}
		watch {
			if delete_action_for_error.error().is_some() {
				div {
					class: "alert-danger mt-3",
					{ delete_action_for_error.error().unwrap_or_default() }
				}
			}
		}
	})(
		delete_action,
		show_delete,
		username,
		content,
		created_at,
		tweet_id,
		liked_signal,
		like_count_signal,
		delete_action_for_click,
		delete_action_for_error,
	)
}

/// Tweet form component using form! macro
///
/// Provides form for creating a new tweet with 280 character limit.
/// Demonstrates the use of derived blocks for computed signals (char_count),
/// watch blocks for reactive UI (character counter with 4-level styling),
/// and state management (loading, error signals).
///
/// # Features demonstrated
/// - `derived` block: Automatically computes `char_count` from content
/// - `watch` block with match expressions: 4-level character counter styling
/// - `state` block: Automatic loading/error signal management
/// - `on_success` callback: Page reload after successful submission
pub fn tweet_form() -> Page {
	// Define the form using form! macro with derived signals
	let tweet_form_instance = form! {
		name: TweetFormInner,
		server_fn: create_tweet,
		method: Post,

		// State management - generates loading and error signals automatically
		state: { loading, error },

		fields: {
			content: TextField {
				widget: Textarea,
				bind: true,
				max_length: 280,
				required,
				placeholder: "What's happening?",
				class: "form-textarea border-0 bg-transparent focus:ring-0 text-lg resize-none",
				rows: 3,
			},
		},

		// Watch blocks for reactive UI rendering
		// Following polls.rs pattern: simple inline conditionals without nested watch blocks
		watch: {
			// Character counter with styling based on count
			char_counter: |form| {
				let char_count = form.content().get().len();
				let progress_percent = (char_count as f64 / 280.0 * 100.0).min(100.0);
				let width_style = format!("width: {}%", progress_percent);
				// Determine color class based on count (use String for 'static lifetime)
				let (text_class, bar_class) = if char_count > 280 {
					("text-sm font-medium text-danger".to_string(), "h-full bg-danger transition-all".to_string())
				} else if char_count > 250 {
					("text-sm font-medium text-warning".to_string(), "h-full bg-warning transition-all".to_string())
				} else if char_count > 0 {
					("text-sm font-medium text-content-tertiary".to_string(), "h-full bg-brand transition-all".to_string())
				} else {
					("text-sm font-medium text-content-tertiary".to_string(), "h-full bg-surface-tertiary transition-all".to_string())
				};
				let display_text = format!("{}/280", char_count);
				page!(|text_class: String, bar_class: String, width_style: String, display_text: String| {
					div {
						class: "flex items-center gap-2",
						div {
							class: text_class,
							{ display_text }
						}
						div {
							class: "w-20 h-1 bg-surface-tertiary rounded-full overflow-hidden",
							div {
								class: bar_class,
								style: width_style,
							}
						}
					}
				})(text_class, bar_class, width_style, display_text)
			},
			// Submit button with loading/disabled states
			// Pattern from polls.rs: simple inline conditionals
			submit_button: |form| {
				let is_loading = form.loading().get();
				let char_count = form.content().get().len();
				let is_valid = char_count > 0 && char_count <= 280;
				let is_disabled = is_loading || !is_valid;
				page!(|is_loading: bool, is_disabled: bool| {
					div {
						button {
							type: "submit",
							class: if is_disabled { "btn-primary opacity-50 cursor-not-allowed" } else { "btn-primary" },
							disabled: is_disabled,
							{ if is_loading { "Posting..." } else { "Post" } }
						}
					}
				})(is_loading, is_disabled)
			},
			// Error display - following polls.rs pattern with simple conditional
			error_display: |form| {
				let err = form.error().get();
				let has_error = err.is_some();
				let error_msg = err.unwrap_or_default();
				page!(|has_error: bool, error_msg: String| {
					div {
						class: if has_error { "alert-danger mb-3" } else { "hidden" },
						{ error_msg }
					}
				})(has_error, error_msg)
			},
		},

		// Callback for successful submission - reload page
		on_success: |_result| {
			#[cfg(wasm)]
			{
				if let Some(window) = web_sys::window() {
					let _ = window.location().reload();
				}
			}
		},
	};

	// Wrap form in the card layout
	// Extract the form instance's view components for custom layout
	let form_view = tweet_form_instance.into_page();

	// Create the full card layout
	page!(|form_view: Page| {
		div {
			class: "card mb-4",
			div {
				class: "card-body",
				div {
					class: "flex gap-3",
					div {
						class: "flex-shrink-0 hidden sm:block",
						div {
							class: "tweet-avatar bg-brand/20 flex items-center justify-center text-brand font-semibold",
							"✏️"
						}
					}
					div {
						class: "flex-1",
						{ form_view }
					}
				}
			}
		}
	})(form_view)
}

/// Tweet list component using hooks
///
/// Displays list of tweets with loading and error states.
/// Uses React-like hooks for state management.
/// Uses watch blocks for reactive UI updates when async data loads.
pub fn tweet_list(user_id: Option<Uuid>) -> Page {
	// Data fetching with create_resource on client, initial loading state on server
	let (tweets, _set_tweets) = use_state(Vec::<TweetInfo>::new());
	let (loading, _set_loading) = use_state(true);
	let (error, _set_error) = use_state(None::<String>);

	#[cfg(wasm)]
	{
		let resource = create_resource(move || async move {
			list_tweets(user_id, 0).await.map_err(|e| e.to_string())
		});

		// Bridge resource state to individual signals for page! macro compatibility
		let tweets_setter = _set_tweets.clone();
		let loading_setter = _set_loading.clone();
		let error_setter = _set_error.clone();
		let resource_for_effect = resource.clone();

		use_effect(move || match resource_for_effect.get() {
			ResourceState::Loading => {
				loading_setter(true);
				error_setter(None);
			}
			ResourceState::Success(data) => {
				tweets_setter(data);
				loading_setter(false);
				error_setter(None);
			}
			ResourceState::Error(err) => {
				error_setter(Some(err));
				loading_setter(false);
			}
		});
	}

	// Clone signals for passing to page! macro (NOT extracting values)
	let tweets_signal = tweets.clone();
	let loading_signal = loading.clone();
	let error_signal = error.clone();

	page!(|tweets_signal: Signal<Vec<TweetInfo>>, loading_signal: Signal<bool>, error_signal: Signal<Option<String>>| {
		div {
			watch {
				if loading_signal.get() {
					div {
						class: "flex flex-col items-center justify-center py-12",
						div {
							class: "spinner-lg mb-4",
						}
						p {
							class: "text-content-secondary text-sm",
							"Loading tweets..."
						}
					}
				} else if error_signal.get().is_some() {
					div {
						class: "alert-danger",
						role: "alert",
						div {
							class: "flex items-center gap-2",
							{ icons::error_circle_icon() }
							span {
								{ error_signal.get().unwrap_or_default() }
							}
						}
					}
				} else if tweets_signal.get().is_empty() {
					div {
						class: "flex flex-col items-center justify-center py-16 text-center",
						div {
							class: "w-16 h-16 rounded-full bg-surface-tertiary flex items-center justify-center mb-4",
							{ icons::chat_bubble_icon_lg() }
						}
						h3 {
							class: "text-lg font-semibold text-content-primary mb-1",
							"No tweets yet"
						}
						p {
							class: "text-content-secondary",
							"Be the first to share something!"
						}
					}
				} else {
					div {
						class: "card overflow-hidden",
						{
							Page::Fragment(
									tweets_signal
										.get()
										.iter()
										.map(|t| tweet_card(t, false))
										.collect::<Vec<_>>(),
								)
						}
					}
				}
			}
		}
	})(tweets_signal, loading_signal, error_signal)
}
