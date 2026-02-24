//!
//! Convenient shortcut functions for common Reinhardt operations.
//!
//! Equivalent to Django's `django.shortcuts` module.
//!
//! ## Core Shortcuts
//!
//! ### Rendering Responses
//!
//! ```
//! use reinhardt_shortcuts::{render_json, render_html, render_text};
//! use serde_json::json;
//!
//! // Render JSON response (returns Result to ensure atomic output on error)
//! let data = json!({"status": "success"});
//! let response = render_json(&data).unwrap();
//!
//! // Render HTML response
//! let response = render_html("<h1>Hello</h1>");
//!
//! // Render text response
//! let response = render_text("Plain text");
//! ```
//!
//! ### Redirects
//!
//! ```
//! use reinhardt_shortcuts::{redirect, redirect_permanent};
//! use std::collections::HashSet;
//!
//! let allowed_hosts: HashSet<String> = HashSet::new();
//!
//! // Temporary redirect (302) - validates URL against allowed hosts
//! let response = redirect("/users/", &allowed_hosts).unwrap();
//!
//! // Permanent redirect (301)
//! let response = redirect_permanent("/new-location/", &allowed_hosts).unwrap();
//! ```
//!
//! ### Database Shortcuts
//!
//! ```rust,no_run,ignore
//! # use reinhardt_shortcuts::get_object_or_404;
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # struct User;
//! # let id = 1;
//! // Get object or return 404 (requires "database" feature)
//! let user = get_object_or_404::<User>(id).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Server-Side Rendering
//!
//! For server-side rendering with components, use `reinhardt-pages` directly:
//!
//! ```rust,ignore
//! use reinhardt_pages::{SsrRenderer, SsrOptions, Component};
//!
//! let mut renderer = SsrRenderer::with_options(
//!     SsrOptions::new()
//!         .title("My Page")
//!         .css("/styles.css")
//! );
//! let html = renderer.render_page(&my_component);
//! ```
//!
//! ### Security Headers
//!
//! ```
//! use reinhardt_shortcuts::{render_html, security_headers};
//!
//! // Apply common security headers to any response
//! let response = render_html("<h1>Hello</h1>");
//! let response = security_headers(response);
//! ```
//!
//! ## Implemented Features
//!
//! - ✅ JSON/HTML/Text response rendering
//! - ✅ Redirect shortcuts (302, 301)
//! - ✅ URL utilities
//! - ✅ Database shortcuts (get_object_or_404, get_list_or_404)
//! - ✅ Security headers helper (X-Content-Type-Options, X-Frame-Options, Referrer-Policy, X-XSS-Protection)

pub mod context;
pub mod get_or_404;
pub mod redirect;
pub mod render;
pub mod security_headers;
pub mod url;

// ORM integration (feature-gated)
#[cfg(feature = "database")]
pub mod orm;

// Re-export core functions
pub use context::{ContextError, TemplateContext};
pub use get_or_404::{
	GetError, exists_or_404_response, get_list_or_404_response, get_or_404_response,
};
pub use redirect::{redirect, redirect_permanent, redirect_permanent_to, redirect_to};
pub use reinhardt_core::security::redirect::RedirectValidationError;
pub use render::{
	escape_html, render_html, render_html_safe, render_json, render_json_pretty, render_text,
};
pub use security_headers::security_headers;
pub use url::{Url, UrlError};

// Re-export ORM functions (feature-gated)
#[cfg(feature = "database")]
pub use orm::{get_list_or_404, get_object_or_404};
