//! Introspect management command
//!
//! Outputs structured metadata about the project including app info,
//! databases, routes, middleware, settings, and feature flags.
//! Designed for PaaS platforms to automatically infer resource requirements.

use crate::base::BaseCommand;
use crate::{CommandContext, CommandResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Top-level introspect output structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntrospectOutput {
	/// Application metadata from Cargo.toml
	pub app: AppMetadata,

	/// Configured database connections
	pub databases: Vec<DatabaseMetadata>,

	/// Registered URL routes
	pub routes: Vec<RouteMetadata>,

	/// Registered middleware stack
	pub middleware: Vec<MiddlewareMetadata>,

	/// Application settings summary
	pub settings: SettingsMetadata,

	/// Resolved Cargo features and infrastructure signals
	pub features: FeaturesMetadata,
}

/// Application metadata extracted from Cargo.toml via `cargo_metadata`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMetadata {
	/// Package name
	pub name: String,

	/// Package version
	pub version: String,
}

/// Database connection metadata (passwords are never included)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseMetadata {
	/// Database alias (e.g., "default")
	pub alias: String,

	/// Database engine (e.g., "postgresql", "sqlite")
	pub engine: String,

	/// All registered models in the project.
	///
	/// Note: The model registry does not track per-database routing, so
	/// every database alias reports the same global model list. Multi-database
	/// routing is handled at runtime by the ORM layer, not at introspection time.
	pub tables: Vec<TableMetadata>,
}

/// Table/model metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableMetadata {
	/// Database table name
	pub name: String,

	/// Application label this model belongs to
	pub app: String,
}

/// Route metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteMetadata {
	/// Full URL path pattern
	pub path: String,

	/// Allowed HTTP methods (empty means all methods)
	pub methods: Vec<String>,

	/// Route name for URL reversal
	pub name: Option<String>,

	/// Route namespace
	pub namespace: Option<String>,
}

/// Middleware metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiddlewareMetadata {
	/// Short middleware name
	pub name: String,

	/// Full type path
	pub type_name: String,
}

/// Application settings summary (sensitive values redacted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsMetadata {
	/// Server configuration
	pub server: ServerSettings,

	/// Security configuration
	pub security: SecuritySettings,
}

/// Server-related settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
	/// Default port (derived from common configuration)
	pub default_port: u16,

	/// Debug mode enabled
	pub debug: bool,
}

/// Security-related settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySettings {
	/// SSL redirect enabled
	pub ssl_redirect: bool,

	/// Session cookie secure flag
	pub session_cookie_secure: bool,

	/// CSRF cookie secure flag
	pub csrf_cookie_secure: bool,

	/// HSTS enabled
	pub hsts_enabled: bool,
}

/// Cargo feature metadata for infrastructure signal detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeaturesMetadata {
	/// Features declared by the user in Cargo.toml dependency
	pub declared: Vec<String>,

	/// All resolved features (after Cargo feature unification)
	pub resolved: Vec<String>,

	/// Infrastructure signals inferred from features
	pub infrastructure_signals: InfraSignals,
}

/// Infrastructure requirements inferred from Cargo features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfraSignals {
	/// Database backend type
	pub database: String,

	/// Cache backend type
	pub cache: String,

	/// WebSocket support required
	pub websocket: bool,

	/// Background worker support required
	pub background_worker: bool,

	/// gRPC support required
	pub grpc: bool,

	/// Storage backend type (e.g., "s3", "azure", "gcs")
	pub storage: Option<String>,

	/// Mail backend type (e.g., "smtp")
	pub mail: Option<String>,

	/// Session backend type (e.g., "redis", "database", "file")
	pub session_backend: Option<String>,

	/// GraphQL support required
	pub graphql: bool,

	/// Admin panel support required
	pub admin_panel: bool,

	/// Internationalization support required
	pub i18n: bool,
}

/// Introspect management command
pub struct IntrospectCommand;

#[async_trait]
impl BaseCommand for IntrospectCommand {
	fn name(&self) -> &str {
		"introspect"
	}

	fn description(&self) -> &str {
		"Output structured project metadata (YAML/JSON) for platform introspection"
	}

	async fn execute(&self, _ctx: &CommandContext) -> CommandResult<()> {
		// Execution is handled by execute_introspect in cli.rs
		// which has access to format/section arguments
		Ok(())
	}
}

/// Collect all introspect metadata into the output structure
pub fn collect_introspect_data() -> Result<IntrospectOutput, Box<dyn std::error::Error>> {
	let app = collect_app_metadata()?;
	let databases = collect_database_metadata();
	let routes = collect_route_metadata();
	let middleware = collect_middleware_metadata();
	let settings = collect_settings_metadata();
	let features = collect_features_metadata();

	Ok(IntrospectOutput {
		app,
		databases,
		routes,
		middleware,
		settings,
		features,
	})
}

/// Collect app metadata from cargo_metadata
fn collect_app_metadata() -> Result<AppMetadata, Box<dyn std::error::Error>> {
	let metadata = cargo_metadata::MetadataCommand::new().exec()?;

	if let Some(root) = metadata.root_package() {
		Ok(AppMetadata {
			name: root.name.to_string(),
			version: root.version.to_string(),
		})
	} else {
		// Fallback when running outside a cargo project
		Ok(AppMetadata {
			name: "unknown".to_string(),
			version: "0.0.0".to_string(),
		})
	}
}

/// Collect database metadata from settings and model registry
fn collect_database_metadata() -> Vec<DatabaseMetadata> {
	use reinhardt_apps::registry::get_registered_models;

	// Try to load settings for database configuration
	let databases = load_settings_databases();

	if databases.is_empty() {
		// No databases configured, still collect models under "default"
		let models = get_registered_models();
		if models.is_empty() {
			return Vec::new();
		}

		return vec![DatabaseMetadata {
			alias: "default".to_string(),
			engine: "unknown".to_string(),
			tables: models
				.iter()
				.map(|m| TableMetadata {
					name: m.table_name.to_string(),
					app: m.app_label.to_string(),
				})
				.collect(),
		}];
	}

	let models = get_registered_models();
	let model_tables: Vec<TableMetadata> = models
		.iter()
		.map(|m| TableMetadata {
			name: m.table_name.to_string(),
			app: m.app_label.to_string(),
		})
		.collect();

	// Attach models to the "default" database only, since the model registry
	// does not track per-database routing. Other aliases get empty tables.
	databases
		.into_iter()
		.map(|(alias, engine)| {
			let tables = if alias == "default" {
				model_tables.clone()
			} else {
				Vec::new()
			};
			DatabaseMetadata {
				alias,
				engine,
				tables,
			}
		})
		.collect()
}

/// Build a `SettingsBuilder` with all default values for introspection.
///
/// This avoids duplicating the default-value configuration across multiple
/// call sites. The caller provides `base_dir` and `settings_dir` so that
/// file-based sources can be added.
#[allow(deprecated)] // Uses Settings which is deprecated; retained for backward compatibility
fn build_settings(
	base_dir: &std::path::Path,
	settings_dir: &std::path::Path,
	profile: reinhardt_conf::settings::profile::Profile,
	profile_str: &str,
) -> Result<reinhardt_conf::Settings, Box<dyn std::error::Error>> {
	use reinhardt_conf::settings::builder::SettingsBuilder;
	use reinhardt_conf::settings::sources::{DefaultSource, LowPriorityEnvSource, TomlFileSource};

	let base_dir_str = base_dir
		.to_str()
		.ok_or_else(|| format!("base_dir contains invalid UTF-8: {}", base_dir.display()))?;

	// Generate a random secret key to avoid shipping a hardcoded value,
	// consistent with the approach used in execute_collectstatic.
	let default_secret_key = crate::cli::generate_random_secret_key();

	let merged = SettingsBuilder::new()
		.profile(profile)
		.add_source(
			DefaultSource::new()
				.with_value(
					"base_dir",
					serde_json::Value::String(base_dir_str.to_string()),
				)
				.with_value("debug", serde_json::Value::Bool(true))
				.with_value("secret_key", serde_json::Value::String(default_secret_key))
				.with_value("allowed_hosts", serde_json::Value::Array(vec![]))
				.with_value("installed_apps", serde_json::Value::Array(vec![]))
				.with_value("databases", serde_json::json!({}))
				.with_value("templates", serde_json::Value::Array(vec![]))
				.with_value(
					"static_url",
					serde_json::Value::String("/static/".to_string()),
				)
				.with_value(
					"static_root",
					serde_json::Value::String(
						base_dir.join("staticfiles").to_string_lossy().to_string(),
					),
				)
				.with_value("staticfiles_dirs", serde_json::Value::Array(vec![]))
				.with_value(
					"media_url",
					serde_json::Value::String("/media/".to_string()),
				)
				.with_value(
					"language_code",
					serde_json::Value::String("en-us".to_string()),
				)
				.with_value("time_zone", serde_json::Value::String("UTC".to_string()))
				.with_value("use_i18n", serde_json::Value::Bool(false))
				.with_value("use_tz", serde_json::Value::Bool(false))
				.with_value(
					"default_auto_field",
					serde_json::Value::String("reinhardt.db.models.BigAutoField".to_string()),
				)
				.with_value("secure_ssl_redirect", serde_json::Value::Bool(false))
				.with_value(
					"secure_hsts_include_subdomains",
					serde_json::Value::Bool(false),
				)
				.with_value("secure_hsts_preload", serde_json::Value::Bool(false))
				.with_value("session_cookie_secure", serde_json::Value::Bool(false))
				.with_value("csrf_cookie_secure", serde_json::Value::Bool(false))
				.with_value("append_slash", serde_json::Value::Bool(false))
				.with_value("middleware", serde_json::Value::Array(vec![]))
				.with_value("root_urlconf", serde_json::Value::String(String::new()))
				.with_value("media_root", serde_json::Value::Null)
				.with_value("admins", serde_json::Value::Array(vec![]))
				.with_value("managers", serde_json::Value::Array(vec![])),
		)
		.add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
		.add_source(TomlFileSource::new(settings_dir.join("base.toml")))
		.add_source(TomlFileSource::new(
			settings_dir.join(format!("{}.toml", profile_str)),
		))
		.build()?;

	Ok(merged.into_typed::<reinhardt_conf::Settings>()?)
}

/// Load database configurations from settings, returning (alias, engine) pairs.
/// Returns empty vec if settings cannot be loaded.
#[allow(deprecated)] // Uses Settings which is deprecated; retained for backward compatibility
fn load_settings_databases() -> Vec<(String, String)> {
	use reinhardt_conf::settings::profile::Profile;

	let profile_str = std::env::var("REINHARDT_ENV").unwrap_or_else(|_| "local".to_string());
	let profile = Profile::parse(&profile_str);

	let base_dir = match std::env::current_dir() {
		Ok(dir) => dir,
		Err(_) => return Vec::new(),
	};
	let settings_dir = base_dir.join("settings");

	let settings = match build_settings(&base_dir, &settings_dir, profile, &profile_str) {
		Ok(s) => s,
		Err(_) => return Vec::new(),
	};

	settings
		.core
		.databases
		.iter()
		.map(|(alias, config)| (alias.clone(), config.engine.clone()))
		.collect()
}

/// Collect route metadata from the global router
fn collect_route_metadata() -> Vec<RouteMetadata> {
	if !reinhardt_urls::routers::is_router_registered() {
		return Vec::new();
	}

	let router = match reinhardt_urls::routers::get_router() {
		Some(r) => r,
		None => return Vec::new(),
	};

	router
		.get_all_routes()
		.into_iter()
		.map(|(path, name, namespace, methods)| RouteMetadata {
			path,
			methods: methods.iter().map(|m| m.to_string()).collect(),
			name,
			namespace,
		})
		.collect()
}

/// Collect middleware metadata from the global router
fn collect_middleware_metadata() -> Vec<MiddlewareMetadata> {
	if !reinhardt_urls::routers::is_router_registered() {
		return Vec::new();
	}

	let router = match reinhardt_urls::routers::get_router() {
		Some(r) => r,
		None => return Vec::new(),
	};

	router
		.get_registered_middleware()
		.into_iter()
		.map(|info| MiddlewareMetadata {
			name: info.name,
			type_name: info.type_name,
		})
		.collect()
}

/// Collect settings metadata (sensitive values are never included)
fn collect_settings_metadata() -> SettingsMetadata {
	let (ssl_redirect, session_cookie_secure, csrf_cookie_secure, hsts_enabled, debug) =
		load_security_settings();

	SettingsMetadata {
		server: ServerSettings {
			default_port: 8000,
			debug,
		},
		security: SecuritySettings {
			ssl_redirect,
			session_cookie_secure,
			csrf_cookie_secure,
			hsts_enabled,
		},
	}
}

/// Load security-related settings. Returns defaults if settings cannot be loaded.
#[allow(deprecated)] // Uses Settings which is deprecated; retained for backward compatibility
fn load_security_settings() -> (bool, bool, bool, bool, bool) {
	use reinhardt_conf::settings::profile::Profile;

	let profile_str = std::env::var("REINHARDT_ENV").unwrap_or_else(|_| "local".to_string());
	let profile = Profile::parse(&profile_str);

	let base_dir = match std::env::current_dir() {
		Ok(dir) => dir,
		Err(_) => return (false, false, false, false, true),
	};
	let settings_dir = base_dir.join("settings");

	match build_settings(&base_dir, &settings_dir, profile, &profile_str) {
		Ok(s) => (
			s.core.security.secure_ssl_redirect,
			s.core.security.session_cookie_secure,
			s.core.security.csrf_cookie_secure,
			s.core.security.secure_hsts_seconds.unwrap_or(0) > 0,
			s.core.debug,
		),
		Err(_) => (false, false, false, false, true),
	}
}

/// Collect feature metadata from cargo_metadata resolve graph
fn collect_features_metadata() -> FeaturesMetadata {
	let metadata = match cargo_metadata::MetadataCommand::new().exec() {
		Ok(m) => m,
		Err(_) => {
			return FeaturesMetadata {
				declared: Vec::new(),
				resolved: Vec::new(),
				infrastructure_signals: InfraSignals {
					database: "none".to_string(),
					cache: "none".to_string(),
					websocket: false,
					background_worker: false,
					grpc: false,
					storage: None,
					mail: None,
					session_backend: None,
					graphql: false,
					admin_panel: false,
					i18n: false,
				},
			};
		}
	};

	let root_package = match metadata.root_package() {
		Some(p) => p,
		None => {
			return FeaturesMetadata {
				declared: Vec::new(),
				resolved: Vec::new(),
				infrastructure_signals: InfraSignals {
					database: "none".to_string(),
					cache: "none".to_string(),
					websocket: false,
					background_worker: false,
					grpc: false,
					storage: None,
					mail: None,
					session_backend: None,
					graphql: false,
					admin_panel: false,
					i18n: false,
				},
			};
		}
	};

	// Find reinhardt dependency and its declared features
	let declared: Vec<String> = root_package
		.dependencies
		.iter()
		.filter(|dep| dep.name == "reinhardt" || dep.name.starts_with("reinhardt-"))
		.flat_map(|dep| dep.features.clone())
		.collect();

	// Get resolved features from the resolve graph
	let resolved: Vec<String> = metadata
		.resolve
		.as_ref()
		.and_then(|resolve| {
			resolve.nodes.iter().find(|node| {
				node.id.repr.contains("reinhardt") && !node.id.repr.contains("reinhardt-test")
			})
		})
		.map(|node| node.features.iter().map(|f| f.to_string()).collect())
		.unwrap_or_default();

	// Combine all features for signal detection
	let all_features: Vec<&str> = declared
		.iter()
		.chain(resolved.iter())
		.map(|s| s.as_str())
		.collect();

	let infrastructure_signals = InfraSignals {
		database: detect_database_signal(&all_features),
		cache: detect_cache_signal(&all_features),
		websocket: all_features
			.iter()
			.any(|f| has_token(f, "websocket") || has_token(f, "websockets")),
		background_worker: all_features
			.iter()
			.any(|f| has_token(f, "tasks") || has_token(f, "worker") || has_token(f, "celery")),
		grpc: all_features.iter().any(|f| has_token(f, "grpc")),
		storage: detect_storage_signal(&all_features),
		mail: detect_mail_signal(&all_features),
		session_backend: detect_session_backend_signal(&all_features),
		graphql: all_features.iter().any(|f| has_token(f, "graphql")),
		admin_panel: all_features.iter().any(|f| has_token(f, "admin")),
		i18n: all_features
			.iter()
			.any(|f| has_token(f, "i18n") || has_token(f, "l10n") || has_token(f, "locale")),
	};

	FeaturesMetadata {
		declared,
		resolved,
		infrastructure_signals,
	}
}

/// Split a feature name into tokens by common separators (`-`, `_`)
fn feature_tokens(feature: &str) -> Vec<&str> {
	feature.split(&['-', '_'][..]).collect()
}

/// Check if a feature name contains a specific token as a whole word
fn has_token(feature: &str, token: &str) -> bool {
	feature_tokens(feature)
		.iter()
		.any(|t| t.eq_ignore_ascii_case(token))
}

/// Detect database type from feature names using strict token matching
fn detect_database_signal(features: &[&str]) -> String {
	for f in features {
		if has_token(f, "postgres") || has_token(f, "postgresql") {
			return "postgresql".to_string();
		}
		if has_token(f, "mysql") {
			return "mysql".to_string();
		}
		if has_token(f, "sqlite") {
			return "sqlite".to_string();
		}
	}
	"none".to_string()
}

/// Detect cache type from feature names using strict token matching
fn detect_cache_signal(features: &[&str]) -> String {
	for f in features {
		if has_token(f, "redis") {
			return "redis".to_string();
		}
		if has_token(f, "memcache") || has_token(f, "memcached") {
			return "memcached".to_string();
		}
	}
	"none".to_string()
}

/// Detect storage backend type from feature names using strict token matching
fn detect_storage_signal(features: &[&str]) -> Option<String> {
	for f in features {
		if has_token(f, "s3") {
			return Some("s3".to_string());
		}
		if has_token(f, "azure") {
			return Some("azure".to_string());
		}
		if has_token(f, "gcs") {
			return Some("gcs".to_string());
		}
	}
	None
}

/// Detect mail backend type from feature names using strict token matching.
///
/// Recognizes both explicit backend tokens (e.g., `smtp`) and the generic
/// `mail` feature flag used by this workspace (`mail = ["reinhardt-mail"]`).
/// When only the generic `mail` token is present, returns `"smtp"` as the
/// default backend since `reinhardt-mail` uses SMTP transport.
fn detect_mail_signal(features: &[&str]) -> Option<String> {
	for f in features {
		if has_token(f, "smtp") {
			return Some("smtp".to_string());
		}
	}
	// Fall back to detecting the generic "mail" feature flag
	for f in features {
		if has_token(f, "mail") {
			return Some("smtp".to_string());
		}
	}
	None
}

/// Detect session backend type from feature names using compound token matching.
///
/// Requires a feature to contain both a session-related token (`session` or
/// `sessions`) and a backend token (e.g., `redis-sessions`, `session-db`) to
/// avoid false positives with cache detection for bare `redis` tokens.
fn detect_session_backend_signal(features: &[&str]) -> Option<String> {
	for f in features {
		let is_session_feature = has_token(f, "session") || has_token(f, "sessions");
		if !is_session_feature {
			continue;
		}
		if has_token(f, "redis") {
			return Some("redis".to_string());
		}
		if has_token(f, "db") || has_token(f, "database") {
			return Some("database".to_string());
		}
		if has_token(f, "file") {
			return Some("file".to_string());
		}
	}
	None
}

/// Format the introspect output as YAML
pub fn format_yaml(output: &IntrospectOutput) -> Result<String, Box<dyn std::error::Error>> {
	Ok(serde_yaml::to_string(output)?)
}

/// Format the introspect output as JSON
pub fn format_json(output: &IntrospectOutput) -> Result<String, Box<dyn std::error::Error>> {
	Ok(serde_json::to_string_pretty(output)?)
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	/// Helper to create a default FeaturesMetadata for tests
	fn default_features() -> FeaturesMetadata {
		FeaturesMetadata {
			declared: vec!["full".to_string()],
			resolved: vec!["postgres".to_string(), "server".to_string()],
			infrastructure_signals: InfraSignals {
				database: "postgresql".to_string(),
				cache: "none".to_string(),
				websocket: false,
				background_worker: false,
				grpc: false,
				storage: None,
				mail: None,
				session_backend: None,
				graphql: false,
				admin_panel: false,
				i18n: false,
			},
		}
	}

	/// Helper to create a minimal FeaturesMetadata for tests
	fn empty_features() -> FeaturesMetadata {
		FeaturesMetadata {
			declared: Vec::new(),
			resolved: Vec::new(),
			infrastructure_signals: InfraSignals {
				database: "none".to_string(),
				cache: "none".to_string(),
				websocket: false,
				background_worker: false,
				grpc: false,
				storage: None,
				mail: None,
				session_backend: None,
				graphql: false,
				admin_panel: false,
				i18n: false,
			},
		}
	}

	#[rstest]
	fn test_introspect_output_serializes_to_yaml() {
		// Arrange
		let output = IntrospectOutput {
			app: AppMetadata {
				name: "test-app".to_string(),
				version: "1.0.0".to_string(),
			},
			databases: vec![DatabaseMetadata {
				alias: "default".to_string(),
				engine: "postgresql".to_string(),
				tables: vec![TableMetadata {
					name: "users".to_string(),
					app: "auth".to_string(),
				}],
			}],
			routes: vec![RouteMetadata {
				path: "/api/users/".to_string(),
				methods: vec!["GET".to_string()],
				name: Some("users:list".to_string()),
				namespace: Some("api".to_string()),
			}],
			middleware: vec![MiddlewareMetadata {
				name: "LoggingMiddleware".to_string(),
				type_name: "reinhardt_middleware::LoggingMiddleware".to_string(),
			}],
			settings: SettingsMetadata {
				server: ServerSettings {
					default_port: 8000,
					debug: true,
				},
				security: SecuritySettings {
					ssl_redirect: false,
					session_cookie_secure: false,
					csrf_cookie_secure: false,
					hsts_enabled: false,
				},
			},
			features: default_features(),
		};

		// Act
		let yaml = format_yaml(&output);
		let json = format_json(&output);

		// Assert
		assert!(yaml.is_ok(), "YAML serialization should succeed");
		assert!(json.is_ok(), "JSON serialization should succeed");
		let yaml_str = yaml.unwrap();
		assert!(yaml_str.contains("test-app"));
		assert!(yaml_str.contains("postgresql"));
		assert!(yaml_str.contains("/api/users/"));
		assert!(yaml_str.contains("LoggingMiddleware"));
	}

	#[rstest]
	fn test_introspect_output_serializes_to_json() {
		// Arrange
		let output = IntrospectOutput {
			app: AppMetadata {
				name: "my-project".to_string(),
				version: "0.2.0".to_string(),
			},
			databases: vec![],
			routes: vec![],
			middleware: vec![],
			settings: SettingsMetadata {
				server: ServerSettings {
					default_port: 8000,
					debug: false,
				},
				security: SecuritySettings {
					ssl_redirect: true,
					session_cookie_secure: true,
					csrf_cookie_secure: true,
					hsts_enabled: true,
				},
			},
			features: empty_features(),
		};

		// Act
		let json = format_json(&output).unwrap();
		let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

		// Assert
		assert_eq!(parsed["app"]["name"], "my-project");
		assert_eq!(parsed["app"]["version"], "0.2.0");
		assert_eq!(parsed["settings"]["security"]["ssl_redirect"], true);
	}

	#[rstest]
	fn test_introspect_command_name_and_description() {
		// Arrange
		let cmd = IntrospectCommand;

		// Act & Assert
		assert_eq!(cmd.name(), "introspect");
		assert!(!cmd.description().is_empty());
	}

	#[rstest]
	fn test_database_passwords_never_in_output() {
		// Arrange
		let output = IntrospectOutput {
			app: AppMetadata {
				name: "test".to_string(),
				version: "1.0.0".to_string(),
			},
			databases: vec![DatabaseMetadata {
				alias: "default".to_string(),
				engine: "postgresql".to_string(),
				tables: vec![],
			}],
			routes: vec![],
			middleware: vec![],
			settings: SettingsMetadata {
				server: ServerSettings {
					default_port: 8000,
					debug: true,
				},
				security: SecuritySettings {
					ssl_redirect: false,
					session_cookie_secure: false,
					csrf_cookie_secure: false,
					hsts_enabled: false,
				},
			},
			features: empty_features(),
		};

		// Act
		let yaml = format_yaml(&output).unwrap();
		let json = format_json(&output).unwrap();

		// Assert: no password field in output
		assert!(!yaml.contains("password"));
		assert!(!json.contains("password"));
	}

	#[rstest]
	fn test_empty_routes_when_no_router() {
		// Arrange & Act
		let routes = collect_route_metadata();

		// Assert
		assert!(routes.is_empty(), "No routes when router not registered");
	}

	#[rstest]
	fn test_empty_middleware_when_no_router() {
		// Arrange & Act
		let middleware = collect_middleware_metadata();

		// Assert
		assert!(
			middleware.is_empty(),
			"No middleware when router not registered"
		);
	}

	#[rstest]
	fn test_features_metadata_serializes_correctly() {
		// Arrange
		let features = FeaturesMetadata {
			declared: vec!["full".to_string(), "postgres".to_string()],
			resolved: vec![
				"server".to_string(),
				"postgres".to_string(),
				"migrations".to_string(),
			],
			infrastructure_signals: InfraSignals {
				database: "postgresql".to_string(),
				cache: "redis".to_string(),
				websocket: true,
				background_worker: false,
				grpc: true,
				storage: Some("s3".to_string()),
				mail: Some("smtp".to_string()),
				session_backend: Some("redis".to_string()),
				graphql: true,
				admin_panel: false,
				i18n: true,
			},
		};

		// Act
		let json = serde_json::to_string_pretty(&features).unwrap();
		let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

		// Assert
		assert_eq!(parsed["declared"][0], "full");
		assert_eq!(parsed["infrastructure_signals"]["database"], "postgresql");
		assert_eq!(parsed["infrastructure_signals"]["cache"], "redis");
		assert_eq!(parsed["infrastructure_signals"]["websocket"], true);
		assert_eq!(parsed["infrastructure_signals"]["background_worker"], false);
		assert_eq!(parsed["infrastructure_signals"]["grpc"], true);
		assert_eq!(parsed["infrastructure_signals"]["storage"], "s3");
		assert_eq!(parsed["infrastructure_signals"]["mail"], "smtp");
		assert_eq!(parsed["infrastructure_signals"]["session_backend"], "redis");
		assert_eq!(parsed["infrastructure_signals"]["graphql"], true);
		assert_eq!(parsed["infrastructure_signals"]["admin_panel"], false);
		assert_eq!(parsed["infrastructure_signals"]["i18n"], true);
	}

	#[rstest]
	#[case(&["db-postgres", "server"], "postgresql")]
	#[case(&["db-mysql", "server"], "mysql")]
	#[case(&["sqlite", "server"], "sqlite")]
	#[case(&["server", "auth"], "none")]
	#[case(&["jpeg-support"], "none")] // "pg" in "jpeg" must NOT trigger postgresql
	#[case(&["aws-sdk"], "none")] // false positive guard
	fn test_detect_database_signal(#[case] features: &[&str], #[case] expected: &str) {
		// Arrange & Act
		let result = detect_database_signal(features);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case(&["redis-backend", "server"], "redis")]
	#[case(&["memcached", "server"], "memcached")]
	#[case(&["server", "auth"], "none")]
	fn test_detect_cache_signal(#[case] features: &[&str], #[case] expected: &str) {
		// Arrange & Act
		let result = detect_cache_signal(features);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	fn test_section_filter_extracts_app() {
		// Arrange
		let output = IntrospectOutput {
			app: AppMetadata {
				name: "section-test".to_string(),
				version: "1.0.0".to_string(),
			},
			databases: vec![],
			routes: vec![],
			middleware: vec![],
			settings: SettingsMetadata {
				server: ServerSettings {
					default_port: 8000,
					debug: true,
				},
				security: SecuritySettings {
					ssl_redirect: false,
					session_cookie_secure: false,
					csrf_cookie_secure: false,
					hsts_enabled: false,
				},
			},
			features: empty_features(),
		};

		// Act
		let full_value = serde_json::to_value(&output).unwrap();
		let app_section = full_value.get("app").unwrap();

		// Assert
		assert_eq!(app_section["name"], "section-test");
		assert_eq!(app_section["version"], "1.0.0");
	}

	#[rstest]
	fn test_section_filter_extracts_routes() {
		// Arrange
		let output = IntrospectOutput {
			app: AppMetadata {
				name: "test".to_string(),
				version: "1.0.0".to_string(),
			},
			databases: vec![],
			routes: vec![RouteMetadata {
				path: "/api/health/".to_string(),
				methods: vec!["GET".to_string()],
				name: None,
				namespace: None,
			}],
			middleware: vec![],
			settings: SettingsMetadata {
				server: ServerSettings {
					default_port: 8000,
					debug: true,
				},
				security: SecuritySettings {
					ssl_redirect: false,
					session_cookie_secure: false,
					csrf_cookie_secure: false,
					hsts_enabled: false,
				},
			},
			features: empty_features(),
		};

		// Act
		let full_value = serde_json::to_value(&output).unwrap();
		let routes_section = full_value.get("routes").unwrap();

		// Assert
		assert!(routes_section.is_array());
		assert_eq!(routes_section[0]["path"], "/api/health/");
	}

	#[rstest]
	fn test_omitting_section_outputs_full_metadata() {
		// Arrange
		let output = IntrospectOutput {
			app: AppMetadata {
				name: "full-test".to_string(),
				version: "2.0.0".to_string(),
			},
			databases: vec![],
			routes: vec![],
			middleware: vec![],
			settings: SettingsMetadata {
				server: ServerSettings {
					default_port: 8000,
					debug: true,
				},
				security: SecuritySettings {
					ssl_redirect: false,
					session_cookie_secure: false,
					csrf_cookie_secure: false,
					hsts_enabled: false,
				},
			},
			features: empty_features(),
		};

		// Act
		let json = format_json(&output).unwrap();
		let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

		// Assert: all sections present
		assert!(parsed.get("app").is_some());
		assert!(parsed.get("databases").is_some());
		assert!(parsed.get("routes").is_some());
		assert!(parsed.get("middleware").is_some());
		assert!(parsed.get("settings").is_some());
		assert!(parsed.get("features").is_some());
	}

	#[rstest]
	#[case(&["grpc-server"], true)]
	#[case(&["my-grpc-api"], true)]
	#[case(&["server"], false)]
	#[case(&["graphics"], false)] // "grpc" must NOT match "graphics"
	fn test_grpc_detection(#[case] features: &[&str], #[case] expected: bool) {
		// Arrange & Act
		let result = features.iter().any(|f| has_token(f, "grpc"));

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case(&["aws-s3-backend"], Some("s3"))]
	#[case(&["azure-storage"], Some("azure"))]
	#[case(&["gcs-backend"], Some("gcs"))]
	#[case(&["server"], None)]
	#[case(&["s3-azure-gcs"], Some("s3"))] // first match wins
	fn test_detect_storage_signal(#[case] features: &[&str], #[case] expected: Option<&str>) {
		// Arrange & Act
		let result = detect_storage_signal(features);

		// Assert
		assert_eq!(result.as_deref(), expected);
	}

	#[rstest]
	#[case(&["smtp-backend"], Some("smtp"))]
	#[case(&["mail-smtp"], Some("smtp"))]
	#[case(&["mail"], Some("smtp"))] // workspace feature: mail = ["reinhardt-mail"]
	#[case(&["server"], None)]
	fn test_detect_mail_signal(#[case] features: &[&str], #[case] expected: Option<&str>) {
		// Arrange & Act
		let result = detect_mail_signal(features);

		// Assert
		assert_eq!(result.as_deref(), expected);
	}

	#[rstest]
	#[case(&["session-redis"], Some("redis"))]
	#[case(&["session-db"], Some("database"))]
	#[case(&["session-database"], Some("database"))]
	#[case(&["session-file"], Some("file"))]
	#[case(&["redis-sessions"], Some("redis"))] // workspace feature: reinhardt-auth/redis-sessions
	#[case(&["redis-backend"], None)] // bare "redis" without "session" must NOT match
	#[case(&["server"], None)]
	fn test_detect_session_backend_signal(
		#[case] features: &[&str],
		#[case] expected: Option<&str>,
	) {
		// Arrange & Act
		let result = detect_session_backend_signal(features);

		// Assert
		assert_eq!(result.as_deref(), expected);
	}

	#[rstest]
	#[case(&["graphql-api"], true)]
	#[case(&["my-graphql"], true)]
	#[case(&["server"], false)]
	fn test_graphql_detection(#[case] features: &[&str], #[case] expected: bool) {
		// Arrange & Act
		let result = features.iter().any(|f| has_token(f, "graphql"));

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case(&["admin-panel"], true)]
	#[case(&["my-admin"], true)]
	#[case(&["server"], false)]
	fn test_admin_panel_detection(#[case] features: &[&str], #[case] expected: bool) {
		// Arrange & Act
		let result = features.iter().any(|f| has_token(f, "admin"));

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case(&["i18n-support"], true)]
	#[case(&["l10n-backend"], true)]
	#[case(&["locale-data"], true)]
	#[case(&["server"], false)]
	fn test_i18n_detection(#[case] features: &[&str], #[case] expected: bool) {
		// Arrange & Act
		let result = features
			.iter()
			.any(|f| has_token(f, "i18n") || has_token(f, "l10n") || has_token(f, "locale"));

		// Assert
		assert_eq!(result, expected);
	}
}
