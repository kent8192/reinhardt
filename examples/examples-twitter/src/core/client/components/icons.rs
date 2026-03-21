//! Shared SVG icon components
//!
//! Provides reusable SVG icon components used across the Twitter clone application.
//! Each icon is a function that returns a `Page` using the `page!` macro.

use reinhardt::pages::component::Page;
use reinhardt::pages::page;

/// Error circle icon (filled, 20x20)
///
/// Circle with X mark, used in error alerts and error displays.
pub fn error_circle_icon() -> Page {
	page!(|| {
		svg {
			class: "w-5 h-5 flex-shrink-0",
			fill: "currentColor",
			viewBox: "0 0 20 20",
			path {
				fill_rule: "evenodd",
				d: "M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z",
			}
		}
	})()
}

/// Error circle icon with custom class
pub fn error_circle_icon_with_class(class: &str) -> Page {
	let class = class.to_string();
	page!(|class: String| {
		svg {
			class: class,
			fill: "currentColor",
			viewBox: "0 0 20 20",
			path {
				fill_rule: "evenodd",
				d: "M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z",
			}
		}
	})(class)
}

/// Close/X icon (stroke, 24x24)
///
/// Simple X mark, used in dismissible alerts and modals.
pub fn close_icon() -> Page {
	page!(|| {
		svg {
			class: "w-4 h-4",
			fill: "none",
			stroke: "currentColor",
			viewBox: "0 0 24 24",
			path {
				stroke_linecap: "round",
				stroke_linejoin: "round",
				stroke_width: "2",
				d: "M6 18L18 6M6 6l12 12",
			}
		}
	})()
}

/// Success checkmark circle icon (filled, 20x20)
///
/// Circle with checkmark, used in success alerts.
pub fn success_check_icon() -> Page {
	page!(|| {
		svg {
			class: "w-5 h-5 flex-shrink-0",
			fill: "currentColor",
			viewBox: "0 0 20 20",
			path {
				fill_rule: "evenodd",
				d: "M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z",
			}
		}
	})()
}

/// Warning triangle icon (filled, 20x20)
///
/// Triangle with exclamation, used in warning alerts.
pub fn warning_icon() -> Page {
	page!(|| {
		svg {
			class: "w-5 h-5 flex-shrink-0 mt-0.5",
			fill: "currentColor",
			viewBox: "0 0 20 20",
			path {
				fill_rule: "evenodd",
				d: "M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z",
			}
		}
	})()
}

/// Sun icon (stroke, 24x24)
///
/// Sun symbol for light theme indicator.
pub fn sun_icon() -> Page {
	page!(|| {
		svg {
			class: "icon-sun w-5 h-5",
			fill: "none",
			stroke: "currentColor",
			viewBox: "0 0 24 24",
			path {
				stroke_linecap: "round",
				stroke_linejoin: "round",
				stroke_width: "2",
				d: "M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z",
			}
		}
	})()
}

/// Moon icon (stroke, 24x24)
///
/// Moon symbol for dark theme indicator.
pub fn moon_icon() -> Page {
	page!(|| {
		svg {
			class: "icon-moon w-5 h-5",
			fill: "none",
			stroke: "currentColor",
			viewBox: "0 0 24 24",
			path {
				stroke_linecap: "round",
				stroke_linejoin: "round",
				stroke_width: "2",
				d: "M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z",
			}
		}
	})()
}

/// Home icon (stroke, 24x24)
///
/// House symbol for home navigation.
pub fn home_icon() -> Page {
	page!(|| {
		svg {
			class: "w-6 h-6",
			fill: "none",
			stroke: "currentColor",
			viewBox: "0 0 24 24",
			path {
				stroke_linecap: "round",
				stroke_linejoin: "round",
				stroke_width: "2",
				d: "M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6",
			}
		}
	})()
}

/// Search/magnifying glass icon (stroke, 24x24)
///
/// Magnifying glass for search navigation.
pub fn search_icon() -> Page {
	page!(|| {
		svg {
			class: "w-6 h-6",
			fill: "none",
			stroke: "currentColor",
			viewBox: "0 0 24 24",
			path {
				stroke_linecap: "round",
				stroke_linejoin: "round",
				stroke_width: "2",
				d: "M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z",
			}
		}
	})()
}

/// Bell/notification icon (stroke, 24x24)
///
/// Bell symbol for notifications navigation.
pub fn bell_icon() -> Page {
	page!(|| {
		svg {
			class: "w-6 h-6",
			fill: "none",
			stroke: "currentColor",
			viewBox: "0 0 24 24",
			path {
				stroke_linecap: "round",
				stroke_linejoin: "round",
				stroke_width: "2",
				d: "M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9",
			}
		}
	})()
}

/// User icon (stroke, 24x24)
///
/// Person silhouette for user/profile navigation.
pub fn user_icon() -> Page {
	page!(|| {
		svg {
			class: "w-6 h-6",
			fill: "none",
			stroke: "currentColor",
			viewBox: "0 0 24 24",
			path {
				stroke_linecap: "round",
				stroke_linejoin: "round",
				stroke_width: "2",
				d: "M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z",
			}
		}
	})()
}

/// Plus icon (stroke, 24x24)
///
/// Plus sign for compose/create actions.
pub fn plus_icon() -> Page {
	page!(|| {
		svg {
			class: "w-6 h-6",
			fill: "none",
			stroke: "currentColor",
			viewBox: "0 0 24 24",
			path {
				stroke_linecap: "round",
				stroke_linejoin: "round",
				stroke_width: "2",
				d: "M12 4v16m8-8H4",
			}
		}
	})()
}

/// Chat bubble icon (stroke, 24x24)
///
/// Speech bubble with dots, used for reply actions and empty message states.
pub fn chat_bubble_icon() -> Page {
	page!(|| {
		svg {
			class: "w-5 h-5",
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
	})()
}

/// Large chat bubble icon (stroke, 24x24, 8x8 size)
///
/// Larger version of chat bubble for empty state displays.
pub fn chat_bubble_icon_lg() -> Page {
	page!(|| {
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
	})()
}

/// Chat bubble branded icon (stroke, 24x24, 8x8 size, brand color)
///
/// Brand-colored chat bubble for login/register page headers.
pub fn chat_bubble_icon_brand() -> Page {
	page!(|| {
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
	})()
}

/// Arrow left / back icon (stroke, 24x24)
///
/// Left-pointing arrow for back navigation.
pub fn arrow_left_icon() -> Page {
	page!(|| {
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
	})()
}

/// Chevron right icon (stroke, 24x24)
///
/// Right-pointing chevron for list item navigation.
pub fn chevron_right_icon() -> Page {
	page!(|| {
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
	})()
}

/// Heart icon outline (stroke, 24x24)
///
/// Empty heart for unliked state.
pub fn heart_icon_outline() -> Page {
	page!(|| {
		svg {
			class: "w-5 h-5",
			fill: "none",
			stroke: "currentColor",
			viewBox: "0 0 24 24",
			path {
				stroke_linecap: "round",
				stroke_linejoin: "round",
				stroke_width: "1.5",
				d: "M4.318 6.318a4.5 4.5 0 000 6.364L12 20.364l7.682-7.682a4.5 4.5 0 00-6.364-6.364L12 7.636l-1.318-1.318a4.5 4.5 0 00-6.364 0z",
			}
		}
	})()
}

/// Heart icon filled (stroke+fill, 24x24, with animation)
///
/// Filled heart for liked state with animation class.
pub fn heart_icon_filled() -> Page {
	page!(|| {
		svg {
			class: "w-5 h-5 animate-heart",
			fill: "currentColor",
			stroke: "currentColor",
			viewBox: "0 0 24 24",
			path {
				stroke_linecap: "round",
				stroke_linejoin: "round",
				stroke_width: "1.5",
				d: "M4.318 6.318a4.5 4.5 0 000 6.364L12 20.364l7.682-7.682a4.5 4.5 0 00-6.364-6.364L12 7.636l-1.318-1.318a4.5 4.5 0 00-6.364 0z",
			}
		}
	})()
}

/// Trash/delete icon (stroke, 24x24)
///
/// Trash can for delete actions.
pub fn trash_icon() -> Page {
	page!(|| {
		svg {
			class: "w-4 h-4",
			fill: "none",
			stroke: "currentColor",
			viewBox: "0 0 24 24",
			path {
				stroke_linecap: "round",
				stroke_linejoin: "round",
				stroke_width: "2",
				d: "M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16",
			}
		}
	})()
}

/// Retweet icon (stroke, 24x24)
///
/// Circular arrows for retweet action.
pub fn retweet_icon() -> Page {
	page!(|| {
		svg {
			class: "w-5 h-5",
			fill: "none",
			stroke: "currentColor",
			viewBox: "0 0 24 24",
			path {
				stroke_linecap: "round",
				stroke_linejoin: "round",
				stroke_width: "1.5",
				d: "M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15",
			}
		}
	})()
}

/// Share/upload icon (stroke, 24x24)
///
/// Upload arrow for share action.
pub fn share_icon() -> Page {
	page!(|| {
		svg {
			class: "w-5 h-5",
			fill: "none",
			stroke: "currentColor",
			viewBox: "0 0 24 24",
			path {
				stroke_linecap: "round",
				stroke_linejoin: "round",
				stroke_width: "1.5",
				d: "M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12",
			}
		}
	})()
}

/// Location pin icon (stroke, 24x24)
///
/// Map pin with inner circle for location display.
pub fn location_pin_icon() -> Page {
	page!(|| {
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
	})()
}

/// Link/chain icon (stroke, 24x24)
///
/// Interlinked chain for website/URL display.
pub fn link_icon() -> Page {
	page!(|| {
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
	})()
}

/// User add icon (stroke, 24x24, brand color)
///
/// Person silhouette with plus sign for registration page header.
pub fn user_add_icon() -> Page {
	page!(|| {
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
	})()
}

/// Multi-chat bubble icon (stroke, 24x24, large)
///
/// Overlapping chat bubbles for empty conversation list state.
pub fn chat_multi_icon_lg() -> Page {
	page!(|| {
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
	})()
}
