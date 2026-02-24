//! Layout components
//!
//! Provides layout components for the Twitter clone application:
//! - `NavItem` - Navigation item structure
//! - `header` - Top navigation bar
//! - `sidebar` - Side panel with trending topics and suggested users
//! - `footer` - Footer component
//! - `main_layout` - Main layout wrapper with header, sidebar, and content
//! - `bottom_navigation` - Mobile bottom tab navigation
//! - `floating_action_button` - Mobile compose FAB
//!
//! ## Design Note
//!
//! These components use UnoCSS for styling with a modern Threads/Bluesky-inspired
//! design. The layout is responsive with a 3-column design on desktop and
//! single-column with bottom tabs on mobile.

use crate::apps::auth::shared::types::UserInfo;
use crate::core::client::components::common::theme_toggle;
use reinhardt::pages::component::{Component, ElementView, IntoView, View};
use reinhardt::pages::page;
use reinhardt::pages::router::Link;

/// Navigation item for the header menu
#[derive(Debug, Clone)]
pub struct NavItem {
	/// Display label
	pub label: String,
	/// URL path
	pub href: String,
	/// Whether this item is currently active
	pub active: bool,
	/// Icon SVG path (optional)
	pub icon: Option<String>,
}

impl NavItem {
	/// Create a new navigation item
	pub fn new(label: impl Into<String>, href: impl Into<String>) -> Self {
		Self {
			label: label.into(),
			href: href.into(),
			active: false,
			icon: None,
		}
	}

	/// Set whether this item is active
	pub fn active(mut self, active: bool) -> Self {
		self.active = active;
		self
	}

	/// Set icon SVG path
	pub fn icon(mut self, icon: impl Into<String>) -> Self {
		self.icon = Some(icon.into());
		self
	}
}

/// Trending topic for sidebar display
#[derive(Debug, Clone)]
pub struct TrendingTopic {
	/// Topic name (e.g., "#RustLang")
	pub name: String,
	/// Number of tweets
	pub tweet_count: u64,
	/// Category (e.g., "Technology", "Entertainment")
	pub category: Option<String>,
}

/// Suggested user for sidebar display
#[derive(Debug, Clone)]
pub struct SuggestedUser {
	/// User ID
	pub id: uuid::Uuid,
	/// Username
	pub username: String,
	/// User bio (short)
	pub bio: Option<String>,
	/// Avatar URL
	pub avatar_url: Option<String>,
}

/// Header component
///
/// Displays the top navigation bar with site branding and user menu.
/// Modern design with blur background and clean typography.
pub fn header(site_name: &str, current_user: Option<&UserInfo>, nav_items: &[NavItem]) -> View {
	// Desktop navigation links
	let nav_links: Vec<View> = nav_items
		.iter()
		.map(|item| {
			let class = if item.active {
				"nav-link-active"
			} else {
				"nav-link"
			};
			let link_view = Link::new(item.href.clone(), item.label.clone())
				.class(class)
				.render();
			link_view
		})
		.collect();
	let nav_links_view = View::fragment(nav_links);

	// Brand link
	let brand_link = Link::new("/".to_string(), site_name.to_string())
		.class("text-xl font-bold text-content-primary hover:text-brand transition-colors")
		.render();

	// Theme toggle
	let theme_toggle_view = theme_toggle();

	// User menu based on authentication state
	let user_menu = if let Some(user) = current_user {
		let username = format!("@{}", user.username);
		let profile_link = Link::new(format!("/profile/{}", user.id), "Profile".to_string())
			.class("btn-ghost btn-sm")
			.render();

		page!(|username: String, profile_link: View| {
			div {
				class: "flex items-center gap-3",
				span {
					class: "text-content-secondary text-sm hidden md:block",
					{ username }
				}
				{ profile_link }
				a {
					href: "/logout",
					class: "btn-outline btn-sm",
					"Logout"
				}
			}
		})(username, profile_link)
	} else {
		let login_link = Link::new("/login".to_string(), "Login".to_string())
			.class("btn-ghost btn-sm")
			.render();
		let register_link = Link::new("/register".to_string(), "Sign up".to_string())
			.class("btn-primary btn-sm")
			.render();

		page!(|login_link: View, register_link: View| {
			div {
				class: "flex items-center gap-2",
				{ login_link }
				{ register_link }
			}
		})(login_link, register_link)
	};

	page!(|brand_link: View, nav_links_view: View, theme_toggle_view: View, user_menu: View| {
		header {
			class: "nav-container",
			div {
				class: "layout-container",
				div {
					class: "flex items-center justify-between h-14",
					div {
						class: "flex items-center gap-6",
						{ brand_link }
						nav {
							class: "hidden md:flex items-center gap-1",
							{ nav_links_view }
						}
					}
					div {
						class: "flex items-center gap-2",
						{ theme_toggle_view }
						{ user_menu }
					}
				}
			}
		}
	})(brand_link, nav_links_view, theme_toggle_view, user_menu)
}

/// Sidebar component
///
/// Displays trending topics and suggested users in a modern card design.
pub fn sidebar(trending_topics: &[TrendingTopic], suggested_users: &[SuggestedUser]) -> View {
	// Trending topics list
	let topics_list: Vec<View> = trending_topics
		.iter()
		.map(|topic| {
			let href = format!("/search?q={}", topic.name);
			let name = topic.name.clone();
			let category = topic
				.category
				.clone()
				.unwrap_or_else(|| "Trending".to_string());
			let tweets_text = format_count(topic.tweet_count);

			page!(|href: String, name: String, category: String, tweets_text: String| {
				a {
					href: href,
					class: "sidebar-item block",
					div {
						class: "text-content-tertiary text-xs mb-0.5",
						{ category }
					}
					div {
						class: "font-semibold text-content-primary",
						{ name }
					}
					div {
						class: "text-content-tertiary text-xs mt-0.5",
						{ tweets_text }
					}
				}
			})(href, name, category, tweets_text)
		})
		.collect();

	let topics_empty = topics_list.is_empty();
	let topics_view = if topics_empty {
		page!(|| {
			div {
				class: "px-4 py-6 text-center text-content-tertiary text-sm",
				"Nothing trending right now"
			}
		})()
	} else {
		View::fragment(topics_list)
	};

	// Suggested users list
	let users_list: Vec<View> = suggested_users
		.iter()
		.map(|user| {
			let profile_href = format!("/profile/{}", user.id);
			let username = format!("@{}", user.username);
			let has_bio = user.bio.is_some();
			let bio_text = user.bio.clone().unwrap_or_default();
			let avatar_initial = user
				.username
				.chars()
				.next()
				.unwrap_or('U')
				.to_uppercase()
				.to_string();

			page!(|profile_href: String, username: String, has_bio: bool, bio_text: String, avatar_initial: String| {
				div {
					class: "sidebar-item",
					div {
						class: "flex items-center gap-3",
						div {
							class: "w-10 h-10 rounded-full bg-surface-tertiary flex items-center justify-center text-content-secondary font-semibold text-sm",
							{ avatar_initial }
						}
						div {
							class: "flex-1 min-w-0",
							a {
								href: profile_href,
								class: "font-semibold text-content-primary hover:underline block truncate",
								{ username }
							}
							if has_bio {
								p {
									class: "text-content-tertiary text-xs truncate",
									{ bio_text }
								}
							}
						}
						button {
							class: "btn-outline btn-sm flex-shrink-0",
							"Follow"
						}
					}
				}
			})(
				profile_href,
				username,
				has_bio,
				bio_text,
				avatar_initial,
			)
		})
		.collect();

	let users_empty = users_list.is_empty();
	let users_view = if users_empty {
		page!(|| {
			div {
				class: "px-4 py-6 text-center text-content-tertiary text-sm",
				"No suggestions yet"
			}
		})()
	} else {
		View::fragment(users_list)
	};

	page!(|topics_view: View, users_view: View| {
		aside {
			class: "sidebar hidden lg:block",
			div {
				class: "sidebar-card",
				div {
					class: "sidebar-header",
					"Trending"
				}
				{ topics_view }
				a {
					href: "/explore",
					class: "block px-4 py-3 text-brand text-sm hover:bg-surface-secondary transition-colors",
					"Show more"
				}
			}
			div {
				class: "sidebar-card",
				div {
					class: "sidebar-header",
					"Who to follow"
				}
				{ users_view }
				a {
					href: "/explore/users",
					class: "block px-4 py-3 text-brand text-sm hover:bg-surface-secondary transition-colors",
					"Show more"
				}
			}
			div {
				class: "px-4 py-4 text-content-tertiary text-xs",
				div {
					class: "flex flex-wrap gap-x-3 gap-y-1",
					a {
						href: "/about",
						class: "hover:underline",
						"About"
					}
					a {
						href: "/privacy",
						class: "hover:underline",
						"Privacy"
					}
					a {
						href: "/terms",
						class: "hover:underline",
						"Terms"
					}
					a {
						href: "/help",
						class: "hover:underline",
						"Help"
					}
				}
				p {
					class: "mt-2",
					"Built with Reinhardt"
				}
			}
		}
	})(topics_view, users_view)
}

/// Bottom navigation for mobile
fn bottom_navigation(current_path: &str) -> View {
	let is_home = current_path == "/" || current_path.starts_with("/home");
	let is_explore = current_path.starts_with("/explore") || current_path.starts_with("/search");
	let is_notifications = current_path.starts_with("/notifications");
	let is_profile = current_path.starts_with("/profile");

	let home_class = if is_home {
		"bottom-nav-item-active"
	} else {
		"bottom-nav-item"
	};
	let explore_class = if is_explore {
		"bottom-nav-item-active"
	} else {
		"bottom-nav-item"
	};
	let notif_class = if is_notifications {
		"bottom-nav-item-active"
	} else {
		"bottom-nav-item"
	};
	let profile_class = if is_profile {
		"bottom-nav-item-active"
	} else {
		"bottom-nav-item"
	};

	page!(|home_class: String, explore_class: String, notif_class: String, profile_class: String| {
		nav {
			class: "bottom-nav",
			a {
				href: "/",
				class: home_class,
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
			}
			a {
				href: "/explore",
				class: explore_class,
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
			}
			a {
				href: "/notifications",
				class: notif_class,
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
			}
			a {
				href: "/profile",
				class: profile_class,
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
			}
		}
	})(
		home_class.to_string(),
		explore_class.to_string(),
		notif_class.to_string(),
		profile_class.to_string(),
	)
}

/// Floating action button for mobile compose
fn floating_action_button() -> View {
	page!(|| {
		a {
			href: "/compose",
			class: "fab",
			aria_label: "Compose",
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
		}
	})()
}

/// Footer component
///
/// Displays a simple footer for desktop view.
pub fn footer(version: &str) -> View {
	let version = version.to_string();
	page!(|version: String| {
		footer {
			class: "hidden md:block bg-surface-primary border-t border-border py-6 mt-auto",
			div {
				class: "layout-container",
				div {
					class: "flex flex-col md:flex-row justify-between items-center gap-4 text-content-tertiary text-sm",
					div {
						class: "flex flex-wrap justify-center gap-4",
						a {
							href: "/about",
							class: "hover:text-content-primary transition-colors",
							"About"
						}
						a {
							href: "/privacy",
							class: "hover:text-content-primary transition-colors",
							"Privacy Policy"
						}
						a {
							href: "/terms",
							class: "hover:text-content-primary transition-colors",
							"Terms of Service"
						}
						a {
							href: "/help",
							class: "hover:text-content-primary transition-colors",
							"Help Center"
						}
					}
					span {
						{ format!("Twitter Clone v{} - Built with Reinhardt", version) }
					}
				}
			}
		}
	})(version)
}

/// Main layout wrapper
///
/// Wraps content with header, optional sidebar, and footer.
/// Responsive design: 3-column on desktop, single column with bottom tabs on mobile.
pub fn main_layout(
	site_name: &str,
	current_user: Option<&UserInfo>,
	nav_items: &[NavItem],
	content: View,
	show_sidebar: bool,
	version: &str,
) -> View {
	let header_view = header(site_name, current_user, nav_items);
	let footer_view = footer(version);
	let bottom_nav = bottom_navigation("/");
	let fab = floating_action_button();

	// Build main content with conditional sidebar
	let main_content = if show_sidebar {
		ElementView::new("div")
			.attr("class", "flex gap-6")
			.child(
				ElementView::new("div")
					.attr("class", "flex-1 min-w-0 max-w-2xl")
					.child(content),
			)
			.child(
				ElementView::new("div")
					.attr("class", "w-80 flex-shrink-0 hidden lg:block")
					.child(sidebar(&[], &[])),
			)
			.into_view()
	} else {
		ElementView::new("div")
			.attr("class", "max-w-2xl mx-auto")
			.child(content)
			.into_view()
	};

	page!(|header_view: View, main_content: View, footer_view: View, bottom_nav: View, fab: View| {
		div {
			class: "layout-main bg-surface-secondary",
			{ header_view }
			main {
				class: "flex-1 pt-4 pb-20 md:pb-4",
				div {
					class: "layout-container",
					{ main_content }
				}
			}
			{ footer_view }
			{ bottom_nav }
			{ fab }
		}
	})(header_view, main_content, footer_view, bottom_nav, fab)
}

/// Simple page layout without sidebar
///
/// A simplified layout for pages like login/register that don't need sidebar.
pub fn simple_layout(site_name: &str, nav_items: &[NavItem], content: View, version: &str) -> View {
	let header_view = header(site_name, None, nav_items);
	let footer_view = footer(version);

	page!(|header_view: View, content: View, footer_view: View| {
		div {
			class: "layout-main bg-surface-secondary",
			{ header_view }
			main {
				class: "flex-1 py-8",
				div {
					class: "layout-container",
					div {
						class: "max-w-md mx-auto",
						{ content }
					}
				}
			}
			{ footer_view }
		}
	})(header_view, content, footer_view)
}

/// Format large numbers for display (e.g., 1.2K, 3.4M)
fn format_count(count: u64) -> String {
	if count >= 1_000_000 {
		format!("{:.1}M posts", count as f64 / 1_000_000.0)
	} else if count >= 1_000 {
		format!("{:.1}K posts", count as f64 / 1_000.0)
	} else {
		format!("{} posts", count)
	}
}
