//! Admin panel settings
//!
//! Provides [`AdminSettings`], [`AdminCspSettings`], and [`AdminSecuritySettings`]
//! for configuring the admin panel via TOML configuration files.

#[cfg(not(target_arch = "wasm32"))]
mod inner {
	use serde::{Deserialize, Serialize};

	// ============================================================
	// Default value functions
	// ============================================================

	fn default_site_title() -> String {
		"Reinhardt Admin".to_string()
	}

	fn default_site_header() -> String {
		"Administration".to_string()
	}

	fn default_list_per_page() -> usize {
		100
	}

	fn default_login_url() -> String {
		"/admin/login".to_string()
	}

	fn default_logout_url() -> String {
		"/admin/logout".to_string()
	}

	fn default_self_only() -> Vec<String> {
		vec!["'self'".to_string()]
	}

	fn default_script_src() -> Vec<String> {
		vec!["'self'".to_string(), "'wasm-unsafe-eval'".to_string()]
	}

	fn default_style_src() -> Vec<String> {
		vec!["'self'".to_string(), "'unsafe-inline'".to_string()]
	}

	fn default_img_src() -> Vec<String> {
		vec!["'self'".to_string(), "data:".to_string()]
	}

	fn default_frame_ancestors() -> Vec<String> {
		vec!["'none'".to_string()]
	}

	fn default_frame_options() -> String {
		"deny".to_string()
	}

	fn default_referrer_policy() -> String {
		"strict-origin-when-cross-origin".to_string()
	}

	fn default_permissions_policy() -> String {
		"camera=(), microphone=(), geolocation=(), payment=()".to_string()
	}

	// ============================================================
	// AdminCspSettings
	// ============================================================

	/// Content Security Policy settings for the admin panel.
	///
	/// Controls which resources can be loaded by the admin UI.
	/// Default values match [`ContentSecurityPolicy::admin_default()`] in the
	/// security module, ensuring consistency between hardcoded and
	/// configuration-driven CSP.
	///
	/// [`ContentSecurityPolicy::admin_default()`]: crate::server::security::ContentSecurityPolicy::admin_default
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::settings::AdminCspSettings;
	///
	/// let csp = AdminCspSettings::default();
	/// assert_eq!(csp.default_src, vec!["'self'"]);
	/// assert_eq!(csp.script_src, vec!["'self'", "'wasm-unsafe-eval'"]);
	/// ```
	#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
	pub struct AdminCspSettings {
		/// Sources allowed for default resource loading.
		#[serde(default = "default_self_only")]
		pub default_src: Vec<String>,
		/// Sources allowed for script execution.
		#[serde(default = "default_script_src")]
		pub script_src: Vec<String>,
		/// Sources allowed for stylesheets.
		#[serde(default = "default_style_src")]
		pub style_src: Vec<String>,
		/// Sources allowed for images.
		#[serde(default = "default_img_src")]
		pub img_src: Vec<String>,
		/// Sources allowed for fonts.
		#[serde(default = "default_self_only")]
		pub font_src: Vec<String>,
		/// Sources allowed for fetch/XHR/WebSocket connections.
		#[serde(default = "default_self_only")]
		pub connect_src: Vec<String>,
		/// Restricts which origins can embed the page in a frame.
		#[serde(default = "default_frame_ancestors")]
		pub frame_ancestors: Vec<String>,
		/// Restricts base URI for relative URLs.
		#[serde(default = "default_self_only")]
		pub base_uri: Vec<String>,
		/// Restricts form submission targets.
		#[serde(default = "default_self_only")]
		pub form_action: Vec<String>,
	}

	impl Default for AdminCspSettings {
		fn default() -> Self {
			Self {
				default_src: default_self_only(),
				script_src: default_script_src(),
				style_src: default_style_src(),
				img_src: default_img_src(),
				font_src: default_self_only(),
				connect_src: default_self_only(),
				frame_ancestors: default_frame_ancestors(),
				base_uri: default_self_only(),
				form_action: default_self_only(),
			}
		}
	}

	// ============================================================
	// AdminSecuritySettings
	// ============================================================

	/// Security header settings for the admin panel.
	///
	/// Controls HTTP security headers applied to admin responses,
	/// including frame options, referrer policy, and permissions policy.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::settings::AdminSecuritySettings;
	///
	/// let security = AdminSecuritySettings::default();
	/// assert_eq!(security.frame_options, "deny");
	/// assert_eq!(security.referrer_policy, "strict-origin-when-cross-origin");
	/// ```
	#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
	pub struct AdminSecuritySettings {
		/// X-Frame-Options header value (e.g., "deny", "sameorigin").
		#[serde(default = "default_frame_options")]
		pub frame_options: String,
		/// Referrer-Policy header value.
		#[serde(default = "default_referrer_policy")]
		pub referrer_policy: String,
		/// Permissions-Policy header value.
		#[serde(default = "default_permissions_policy")]
		pub permissions_policy: String,
	}

	impl Default for AdminSecuritySettings {
		fn default() -> Self {
			Self {
				frame_options: default_frame_options(),
				referrer_policy: default_referrer_policy(),
				permissions_policy: default_permissions_policy(),
			}
		}
	}

	// ============================================================
	// AdminSettings
	// ============================================================

	/// Top-level admin panel settings.
	///
	/// Combines UI configuration, CSP directives, and security headers
	/// into a single settings struct that can be deserialized from TOML.
	/// All fields have sensible defaults, so partial TOML is fully supported.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::settings::AdminSettings;
	///
	/// let settings = AdminSettings::default();
	/// assert_eq!(settings.site_title, "Reinhardt Admin");
	/// assert_eq!(settings.list_per_page, 100);
	/// ```
	#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
	pub struct AdminSettings {
		/// Title displayed in the admin panel browser tab.
		#[serde(default = "default_site_title")]
		pub site_title: String,
		/// Header text displayed at the top of the admin panel.
		#[serde(default = "default_site_header")]
		pub site_header: String,
		/// Number of items per page in list views.
		#[serde(default = "default_list_per_page")]
		pub list_per_page: usize,
		/// URL path for the admin login page.
		#[serde(default = "default_login_url")]
		pub login_url: String,
		/// URL path for the admin logout page.
		#[serde(default = "default_logout_url")]
		pub logout_url: String,
		/// Content Security Policy settings.
		#[serde(default)]
		pub csp: AdminCspSettings,
		/// Security header settings.
		#[serde(default)]
		pub security: AdminSecuritySettings,
	}

	impl Default for AdminSettings {
		fn default() -> Self {
			Self {
				site_title: default_site_title(),
				site_header: default_site_header(),
				list_per_page: default_list_per_page(),
				login_url: default_login_url(),
				logout_url: default_logout_url(),
				csp: AdminCspSettings::default(),
				security: AdminSecuritySettings::default(),
			}
		}
	}

	#[cfg(test)]
	mod tests {
		use super::*;
		use rstest::rstest;

		// ============================================================
		// Default value tests
		// ============================================================

		#[rstest]
		fn test_admin_settings_default_values() {
			// Arrange & Act
			let settings = AdminSettings::default();

			// Assert
			assert_eq!(settings.site_title, "Reinhardt Admin");
			assert_eq!(settings.site_header, "Administration");
			assert_eq!(settings.list_per_page, 100);
			assert_eq!(settings.login_url, "/admin/login");
			assert_eq!(settings.logout_url, "/admin/logout");
		}

		#[rstest]
		fn test_admin_csp_settings_default_matches_admin_default() {
			// Arrange & Act
			let csp = AdminCspSettings::default();

			// Assert — each field must match ContentSecurityPolicy::admin_default()
			assert_eq!(csp.default_src, vec!["'self'"]);
			assert_eq!(csp.script_src, vec!["'self'", "'wasm-unsafe-eval'"]);
			assert_eq!(csp.style_src, vec!["'self'", "'unsafe-inline'"]);
			assert_eq!(csp.img_src, vec!["'self'", "data:"]);
			assert_eq!(csp.font_src, vec!["'self'"]);
			assert_eq!(csp.connect_src, vec!["'self'"]);
			assert_eq!(csp.frame_ancestors, vec!["'none'"]);
			assert_eq!(csp.base_uri, vec!["'self'"]);
			assert_eq!(csp.form_action, vec!["'self'"]);
		}

		#[rstest]
		fn test_admin_security_settings_default_values() {
			// Arrange & Act
			let security = AdminSecuritySettings::default();

			// Assert
			assert_eq!(security.frame_options, "deny");
			assert_eq!(
				security.referrer_policy,
				"strict-origin-when-cross-origin"
			);
			assert_eq!(
				security.permissions_policy,
				"camera=(), microphone=(), geolocation=(), payment=()"
			);
		}

		// ============================================================
		// TOML deserialization tests
		// ============================================================

		#[rstest]
		fn test_toml_partial_deserialization() {
			// Arrange
			let toml_str = r#"
site_title = "My Admin"
list_per_page = 50
"#;

			// Act
			let settings: AdminSettings = toml::from_str(toml_str).unwrap();

			// Assert — overridden fields
			assert_eq!(settings.site_title, "My Admin");
			assert_eq!(settings.list_per_page, 50);
			// Assert — default fields preserved
			assert_eq!(settings.site_header, "Administration");
			assert_eq!(settings.login_url, "/admin/login");
			assert_eq!(settings.csp, AdminCspSettings::default());
			assert_eq!(settings.security, AdminSecuritySettings::default());
		}

		#[rstest]
		fn test_toml_csp_override() {
			// Arrange
			let toml_str = r#"
[csp]
script_src = ["'self'", "'wasm-unsafe-eval'", "https://cdn.example.com"]
img_src = ["'self'", "data:", "https://images.example.com"]
"#;

			// Act
			let settings: AdminSettings = toml::from_str(toml_str).unwrap();

			// Assert — overridden CSP fields
			assert_eq!(
				settings.csp.script_src,
				vec!["'self'", "'wasm-unsafe-eval'", "https://cdn.example.com"]
			);
			assert_eq!(
				settings.csp.img_src,
				vec!["'self'", "data:", "https://images.example.com"]
			);
			// Assert — non-overridden CSP fields keep defaults
			assert_eq!(settings.csp.default_src, vec!["'self'"]);
			assert_eq!(settings.csp.font_src, vec!["'self'"]);
			assert_eq!(settings.csp.frame_ancestors, vec!["'none'"]);
		}

		#[rstest]
		fn test_toml_empty_deserialization() {
			// Arrange
			let toml_str = "";

			// Act
			let settings: AdminSettings = toml::from_str(toml_str).unwrap();

			// Assert — all defaults applied
			assert_eq!(settings, AdminSettings::default());
		}
	}
}

#[cfg(not(target_arch = "wasm32"))]
pub use inner::*;
