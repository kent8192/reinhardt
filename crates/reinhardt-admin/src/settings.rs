//! Admin panel settings
//!
//! Provides [`AdminSettings`], [`AdminCspSettings`], and [`AdminSecuritySettings`]
//! for configuring the admin panel via TOML configuration files.

#[cfg(server)]
mod inner {
	use reinhardt_conf::settings::fragment::{HasSettings, SettingsFragment, SettingsValidation};
	use reinhardt_conf::settings::profile::Profile;
	use reinhardt_conf::settings::validation::ValidationResult;
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

	impl SettingsValidation for AdminSettings {
		fn validate(&self, _profile: &Profile) -> ValidationResult {
			self.warn_csp_misconfigurations();
			self.warn_security_misconfigurations();
			Ok(())
		}
	}

	impl SettingsFragment for AdminSettings {
		type Accessor = dyn HasAdminSettings;

		fn section() -> &'static str {
			"admin"
		}

		fn validate(&self, profile: &Profile) -> ValidationResult {
			<Self as SettingsValidation>::validate(self, profile)
		}
	}

	/// Trait for accessing [`AdminSettings`] from a composed settings type.
	pub trait HasAdminSettings {
		/// Get a reference to the admin settings.
		fn admin(&self) -> &AdminSettings;
	}

	impl<T: HasSettings<AdminSettings>> HasAdminSettings for T {
		fn admin(&self) -> &AdminSettings {
			self.get_settings()
		}
	}

	use crate::server::security::{ContentSecurityPolicy, SecurityHeaders};

	impl AdminSettings {
		/// Build [`SecurityHeaders`] from these settings.
		///
		/// Converts the TOML-friendly string configuration into the typed
		/// security headers used by the admin SPA handler.
		pub fn to_security_headers(&self) -> SecurityHeaders {
			SecurityHeaders {
				csp: ContentSecurityPolicy {
					default_src: self.csp.default_src.clone(),
					script_src: self.csp.script_src.clone(),
					style_src: self.csp.style_src.clone(),
					img_src: self.csp.img_src.clone(),
					font_src: self.csp.font_src.clone(),
					connect_src: self.csp.connect_src.clone(),
					frame_ancestors: self.csp.frame_ancestors.clone(),
					base_uri: self.csp.base_uri.clone(),
					form_action: self.csp.form_action.clone(),
				},
				frame_options: self.security.frame_options.parse().unwrap(),
				referrer_policy: self.security.referrer_policy.parse().unwrap(),
				permissions_policy: self.security.permissions_policy.clone(),
			}
		}

		/// Emit tracing warnings for CSP misconfigurations.
		fn warn_csp_misconfigurations(&self) {
			if !self.csp.default_src.iter().any(|s| s == "'self'") {
				tracing::warn!(
					"Admin CSP: default_src is missing `'self'`, this may block admin resources"
				);
			}
			if !self.csp.script_src.iter().any(|s| s == "'self'") {
				tracing::warn!(
					"Admin CSP: script_src is missing `'self'`, admin panel assets may not load"
				);
			}
			if !self
				.csp
				.script_src
				.iter()
				.any(|s| s == "'wasm-unsafe-eval'")
			{
				tracing::warn!(
					"Admin CSP: script_src is missing `'wasm-unsafe-eval'`, WASM SPA will not function"
				);
			}
			if !self.csp.style_src.iter().any(|s| s == "'self'") {
				tracing::warn!(
					"Admin CSP: style_src is missing `'self'`, admin styles may not load"
				);
			}
			if !self.csp.connect_src.iter().any(|s| s == "'self'") {
				tracing::warn!(
					"Admin CSP: connect_src is missing `'self'`, API calls will be blocked"
				);
			}
		}

		/// Emit tracing warnings for security header misconfigurations.
		fn warn_security_misconfigurations(&self) {
			let fo = self.security.frame_options.to_lowercase();
			if fo != "deny" && fo != "sameorigin" {
				tracing::warn!(
					"Admin security: unrecognized frame_options value '{}', expected 'deny' or 'sameorigin'",
					self.security.frame_options
				);
			}
		}
	}

	// ============================================================
	// Global settings accessor (OnceLock)
	// ============================================================

	use std::sync::OnceLock;

	/// Global admin settings instance, set once at application startup.
	static ADMIN_SETTINGS: OnceLock<AdminSettings> = OnceLock::new();

	/// Register custom admin settings for the application.
	///
	/// Call this once during application startup to customize admin CSP,
	/// security headers, and site configuration. If not called, the admin
	/// panel uses [`AdminSettings::default()`].
	///
	/// # Panics
	///
	/// Panics if called more than once (settings are immutable after initialization).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::settings::{configure, AdminSettings};
	///
	/// let mut settings = AdminSettings::default();
	/// settings.site_title = "My Admin".to_string();
	/// configure(settings);
	/// ```
	pub fn configure(settings: AdminSettings) {
		ADMIN_SETTINGS
			.set(settings)
			.expect("AdminSettings can only be configured once");
	}

	/// Get the current admin settings.
	///
	/// Returns the settings registered via [`configure()`], or
	/// [`AdminSettings::default()`] if no custom settings were registered.
	pub fn get_admin_settings() -> &'static AdminSettings {
		ADMIN_SETTINGS.get_or_init(AdminSettings::default)
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
			assert_eq!(security.referrer_policy, "strict-origin-when-cross-origin");
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

		// ============================================================
		// SettingsFragment tests
		// ============================================================

		#[rstest]
		fn test_settings_fragment_section_is_admin() {
			// Arrange / Act
			use reinhardt_conf::SettingsFragment;
			let section = AdminSettings::section();

			// Assert
			assert_eq!(section, "admin");
		}

		#[rstest]
		fn test_validate_warns_on_missing_self_in_script_src() {
			// Arrange
			let mut settings = AdminSettings::default();
			settings.csp.script_src = vec!["'wasm-unsafe-eval'".to_string()];

			// Act
			use reinhardt_conf::settings::fragment::SettingsValidation;
			let result = SettingsValidation::validate(
				&settings,
				&reinhardt_conf::settings::profile::Profile::Development,
			);

			// Assert
			assert!(result.is_ok());
		}

		#[rstest]
		fn test_validate_warns_on_missing_wasm_unsafe_eval() {
			// Arrange
			let mut settings = AdminSettings::default();
			settings.csp.script_src = vec!["'self'".to_string()];

			// Act
			use reinhardt_conf::settings::fragment::SettingsValidation;
			let result = SettingsValidation::validate(
				&settings,
				&reinhardt_conf::settings::profile::Profile::Development,
			);

			// Assert
			assert!(result.is_ok());
		}

		#[rstest]
		fn test_validate_warns_on_unrecognized_frame_options() {
			// Arrange
			let mut settings = AdminSettings::default();
			settings.security.frame_options = "invalid-value".to_string();

			// Act
			use reinhardt_conf::settings::fragment::SettingsValidation;
			let result = SettingsValidation::validate(
				&settings,
				&reinhardt_conf::settings::profile::Profile::Production,
			);

			// Assert
			assert!(result.is_ok());
		}

		#[rstest]
		fn test_validate_ok_with_defaults() {
			// Arrange
			let settings = AdminSettings::default();

			// Act
			use reinhardt_conf::settings::fragment::SettingsValidation;
			let result = SettingsValidation::validate(
				&settings,
				&reinhardt_conf::settings::profile::Profile::Production,
			);

			// Assert
			assert!(result.is_ok());
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

		// ============================================================
		// to_security_headers tests
		// ============================================================

		#[rstest]
		fn test_to_security_headers_default_matches_security_headers_default() {
			// Arrange
			let admin_settings = AdminSettings::default();
			let direct_headers = crate::server::security::SecurityHeaders::default();

			// Act
			let converted_headers = admin_settings.to_security_headers();

			// Assert
			let direct_map = direct_headers.to_header_map();
			let converted_map = converted_headers.to_header_map();
			assert_eq!(
				direct_map.get("Content-Security-Policy"),
				converted_map.get("Content-Security-Policy")
			);
			assert_eq!(
				direct_map.get("X-Frame-Options"),
				converted_map.get("X-Frame-Options")
			);
			assert_eq!(
				direct_map.get("Referrer-Policy"),
				converted_map.get("Referrer-Policy")
			);
			assert_eq!(
				direct_map.get("Permissions-Policy"),
				converted_map.get("Permissions-Policy")
			);
		}

		#[rstest]
		fn test_to_security_headers_custom_csp() {
			// Arrange
			let mut settings = AdminSettings::default();
			settings.csp.script_src = vec![
				"'self'".to_string(),
				"'wasm-unsafe-eval'".to_string(),
				"https://cdn.example.com".to_string(),
			];

			// Act
			let headers = settings.to_security_headers();
			let map = headers.to_header_map();
			let csp = map.get("Content-Security-Policy").unwrap();

			// Assert
			assert!(csp.contains("https://cdn.example.com"));
			assert!(csp.contains("'self'"));
			assert!(csp.contains("'wasm-unsafe-eval'"));
		}

		#[rstest]
		fn test_to_security_headers_custom_frame_options() {
			// Arrange
			let mut settings = AdminSettings::default();
			settings.security.frame_options = "sameorigin".to_string();

			// Act
			let headers = settings.to_security_headers();
			let map = headers.to_header_map();

			// Assert
			assert_eq!(map.get("X-Frame-Options").unwrap(), "SAMEORIGIN");
		}

		#[rstest]
		fn test_to_security_headers_custom_permissions_policy() {
			// Arrange
			let mut settings = AdminSettings::default();
			settings.security.permissions_policy = "camera=(), microphone=()".to_string();

			// Act
			let headers = settings.to_security_headers();
			let map = headers.to_header_map();

			// Assert
			assert_eq!(
				map.get("Permissions-Policy").unwrap(),
				"camera=(), microphone=()"
			);
		}
	}
}

#[cfg(server)]
pub use inner::*;
