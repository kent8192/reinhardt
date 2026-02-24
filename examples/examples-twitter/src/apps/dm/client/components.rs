//! DM client components
//!
//! WASM UI components for direct messaging using reactive hooks.
//! Features real-time WebSocket communication for live message updates.

use crate::apps::dm::client::hooks::{use_dm_chat, use_dm_room_list};
use crate::apps::dm::shared::types::{MessageInfo, RoomInfo};
use reinhardt::pages::Signal;
use reinhardt::pages::component::View;
use reinhardt::pages::page;
use reinhardt::pages::reactive::hooks::{ConnectionState, use_state};
use uuid::Uuid;

/// Message input component (extracted to avoid complex closures)
///
/// Provides a text input for typing messages and a send button.
fn message_input(
	input_signal: Signal<String>,
	send_callback: impl Fn(String) + Clone + 'static,
) -> View {
	let input_for_display = input_signal.clone();
	let input_for_change = input_signal.clone();
	let input_for_click = input_signal.clone();

	page!(|input_for_display: Signal<String>, input_for_change: Signal<String>, input_for_click: Signal<String>| {
		div {
			class: "dm-input-container flex gap-2 p-4 border-t border-surface-tertiary",
			input {
				r#type: "text",
				class: "form-control flex-1",
				placeholder: "Type a message...",
				value: input_for_display.get(),
				@input: {
							let input_signal = input_for_change.clone();
							move |event: web_sys::Event| {
								if let Some(target) = event.target() {
									if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
										input_signal.set(input.value());
									}
								}
							}
						},
			}
			button {
				class: "btn-primary",
				r#type: "button",
				@click: {
							let input_signal = input_for_click.clone();
							let send_callback = send_callback.clone();
							move |_event| {
								let content = input_signal.get();
								if !content.trim().is_empty() {
									send_callback(content);
									input_signal.set(String::new());
								}
							}
						},
				"Send"
			}
		}
	})(input_for_display, input_for_change, input_for_click)
}

/// Single message display component
fn message_item(message: &MessageInfo, is_own_message: bool) -> View {
	let content = message.content.clone();
	let sender = message.sender_username.clone();
	let timestamp = message.created_at.clone();
	let align_class = if is_own_message {
		"flex justify-end"
	} else {
		"flex justify-start"
	};
	let bubble_class = if is_own_message {
		"dm-message own bg-brand text-white rounded-2xl rounded-br-sm px-4 py-2 max-w-[70%]"
	} else {
		"dm-message other bg-surface-secondary rounded-2xl rounded-bl-sm px-4 py-2 max-w-[70%]"
	};

	page!(|align_class: &'static str, bubble_class: &'static str, sender: String, content: String, timestamp: String, is_own_message: bool| {
		div {
			class: align_class,
			div {
				class: bubble_class,
				if ! is_own_message {
					div {
						class: "text-xs font-medium text-content-secondary mb-1",
						{ sender }
					}
				}
				p {
					class: "text-sm break-words",
					{ content }
				}
				div {
					class: if is_own_message { "text-xs text-white/70 mt-1 text-right" } else { "text-xs text-content-tertiary mt-1" },
					{ timestamp }
				}
			}
		}
	})(
		align_class,
		bubble_class,
		sender,
		content,
		timestamp,
		is_own_message,
	)
}

/// Connection status indicator component
fn connection_status(is_connected: bool) -> View {
	page!(|is_connected: bool| {
		div {
			class: "flex items-center gap-2 text-sm",
			div {
				class: if is_connected { "w-2 h-2 rounded-full bg-success animate-pulse" } else { "w-2 h-2 rounded-full bg-warning" },
			}
			span {
				class: "text-content-secondary",
				{ if is_connected { "Connected" } else { "Connecting..." } }
			}
		}
	})(is_connected)
}

/// DM chat component
///
/// Provides real-time chat interface for a DM room.
/// Uses WebSocket for live message updates and server functions for initial data.
///
/// # Arguments
///
/// * `room_id` - The UUID of the DM room
/// * `current_user_id` - Optional UUID of the current user (for message alignment)
///
/// # Features
///
/// - Real-time message receiving via WebSocket
/// - Message sending with optimistic UI
/// - Connection status indicator
/// - Loading and error states
pub fn dm_chat(room_id: Uuid, current_user_id: Option<Uuid>) -> View {
	let chat = use_dm_chat(room_id);

	// Local state for input
	let (input, _set_input) = use_state(String::new());

	// Clone signals for page macro
	let messages_signal = chat.messages.clone();
	let is_loading_signal = chat.is_loading.clone();
	let error_signal = chat.error.clone();
	let ws_state = chat.ws.connection_state();
	let input_signal = input.clone();

	// Clone chat handle for send callback
	let chat_for_send = chat.clone();

	page!(|messages_signal: Signal<Vec<MessageInfo>>, is_loading_signal: Signal<bool>, error_signal: Signal<Option<String>>, ws_state: Signal<ConnectionState>, input_signal: Signal<String>, current_user_id: Option<Uuid>, room_id: Uuid| {
		div {
			class: "dm-chat-container flex flex-col h-full",
			// Header with connection status
			div {
				class: "dm-header flex items-center justify-between p-4 border-b border-surface-tertiary",
				h2 {
					class: "text-lg font-semibold",
					"Direct Messages"
				}
				watch {
					{ connection_status(matches!(ws_state.get(), ConnectionState::Open)) }
				}
			}
			// Message list area
			div {
				class: "dm-messages flex-1 overflow-y-auto p-4 space-y-3",
				watch {
					if is_loading_signal.get() {
						div {
							class: "flex flex-col items-center justify-center py-12",
							div {
								class: "spinner-lg mb-4",
							}
							p {
								class: "text-content-secondary text-sm",
								"Loading messages..."
							}
						}
					} else if error_signal.get().is_some() {
						div {
							class: "alert-danger",
							role: "alert",
							{ error_signal.get().unwrap_or_default() }
						}
					} else if messages_signal.get().is_empty() {
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
										d: "M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z",
									}
								}
							}
							h3 {
								class: "text-lg font-semibold text-content-primary mb-1",
								"No messages yet"
							}
							p {
								class: "text-content-secondary",
								"Send a message to start the conversation!"
							}
						}
					} else {
						div {
							class: "space-y-3",
							{
								let current_uid = current_user_id;
								View::fragment(
									messages_signal.get().iter().map(|m| {
										let is_own = current_uid.map(|uid| m.sender_id == uid).unwrap_or(false);
										message_item(m, is_own)
									}).collect::<Vec<_>>()
								)
							}
						}
					}
				}
			}
			// Message input area
			{
				message_input(input_signal.clone(), move |content| {
					chat_for_send.send_message(content);
				})
			}
		}
	})(
		messages_signal,
		is_loading_signal,
		error_signal,
		ws_state,
		input_signal,
		current_user_id,
		room_id,
	)
}

/// Single room item in the list
fn room_item(room: &RoomInfo, on_select: impl Fn(Uuid) + Clone + 'static) -> View {
	let room_id = room.id;
	let name = room.name.clone();
	let last_message = room.last_message.clone();
	let last_activity = room.last_activity.clone();
	let unread_count = room.unread_count;

	page!(|room_id: Uuid, name: String, last_message: Option<String>, last_activity: Option<String>, unread_count: i32| {
		div {
			class: "room-item flex items-center gap-3 p-4 hover:bg-surface-secondary cursor-pointer transition-colors border-b border-surface-tertiary",
			@click: {
						let on_select = on_select.clone();
						move |_event| {
							on_select(room_id);
						}
					},
			div {
				class: "flex-shrink-0",
				div {
					class: "w-12 h-12 rounded-full bg-brand/20 flex items-center justify-center text-brand font-semibold",
					{ name.chars().next().unwrap_or('?').to_uppercase().to_string() }
				}
			}
			div {
				class: "flex-1 min-w-0",
				div {
					class: "flex items-center justify-between",
					span {
						class: "font-semibold text-content-primary truncate",
						{ name.clone() }
					}
					if last_activity.is_some() {
						span {
							class: "text-xs text-content-tertiary",
							{ last_activity.clone().unwrap_or_default() }
						}
					}
				}
				if last_message.is_some() {
					p {
						class: "text-sm text-content-secondary truncate mt-1",
						{ last_message.clone().unwrap_or_default() }
					}
				}
			}
			if unread_count> 0 {
				div {
					class: "flex-shrink-0",
					span {
						class: "inline-flex items-center justify-center w-6 h-6 text-xs font-bold text-white bg-brand rounded-full",
						{ format!("{}", unread_count.min(99)) }
					}
				}
			}
		}
	})(room_id, name, last_message, last_activity, unread_count)
}

/// DM room list component
///
/// Displays a list of DM rooms for the current user.
/// Features real-time unread count updates via WebSocket notifications.
///
/// # Arguments
///
/// * `on_room_select` - Callback function when a room is selected
///
/// # Features
///
/// - Room list with last message preview
/// - Unread message badges (real-time updated)
/// - Loading and error states
pub fn dm_room_list(on_room_select: impl Fn(Uuid) + Clone + 'static) -> View {
	let room_list = use_dm_room_list();

	// Clone signals for page macro
	let rooms_signal = room_list.rooms.clone();
	let is_loading_signal = room_list.is_loading.clone();
	let error_signal = room_list.error.clone();

	page!(|rooms_signal: Signal<Vec<RoomInfo>>, is_loading_signal: Signal<bool>, error_signal: Signal<Option<String>>| {
		div {
			class: "dm-room-list h-full flex flex-col",
			div {
				class: "p-4 border-b border-surface-tertiary",
				h3 {
					class: "text-lg font-semibold",
					"Conversations"
				}
			}
			div {
				class: "flex-1 overflow-y-auto",
				watch {
					if is_loading_signal.get() {
						div {
							class: "flex flex-col items-center justify-center py-12",
							div {
								class: "spinner-lg mb-4",
							}
							p {
								class: "text-content-secondary text-sm",
								"Loading conversations..."
							}
						}
					} else if error_signal.get().is_some() {
						div {
							class: "p-4",
							div {
								class: "alert-danger",
								role: "alert",
								{ error_signal.get().unwrap_or_default() }
							}
						}
					} else if rooms_signal.get().is_empty() {
						div {
							class: "flex flex-col items-center justify-center py-16 text-center px-4",
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
										d: "M17 8h2a2 2 0 012 2v6a2 2 0 01-2 2h-2v4l-4-4H9a1.994 1.994 0 01-1.414-.586m0 0L11 14h4a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2v4l.586-.586z",
									}
								}
							}
							h3 {
								class: "text-lg font-semibold text-content-primary mb-1",
								"No conversations yet"
							}
							p {
								class: "text-content-secondary",
								"Start a new conversation with someone!"
							}
						}
					} else {
						div {
							{ View::fragment(rooms_signal.get().iter().map(|r| { room_item(r, on_room_select.clone()) }).collect ::<Vec<_>>()) }
						}
					}
				}
			}
		}
	})(rooms_signal, is_loading_signal, error_signal)
}
