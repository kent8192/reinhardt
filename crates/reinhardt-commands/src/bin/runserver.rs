//! runserver CLI command
//!
//! Starts the development server.

// Uses deprecated Settings type; retained for backward compatibility until migration is complete.
#![allow(deprecated)]

use clap::Parser;
use colored::Colorize;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode, body::Incoming};
use hyper_util::rt::TokioIo;
use reinhardt_commands::WelcomePage;
use reinhardt_commands::{CollectStaticCommand, CollectStaticOptions};
#[cfg(any(feature = "admin", feature = "pages"))]
use reinhardt_commands::{
	WasmBuildConfig, WasmBuilder, detect_cdylib_in_cargo_toml, is_wasm_stale,
};
use reinhardt_pages::component::Component;
use reinhardt_pages::ssr::SsrRenderer;
use reinhardt_utils::safe_path_join;
use reinhardt_utils::staticfiles::StaticFilesConfig;
use rustls::ServerConfig;
use rustls_pemfile::{certs, private_key};
use std::convert::Infallible;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use reinhardt_conf::settings::builder::SettingsBuilder;
use reinhardt_conf::settings::core_settings::CoreSettings;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::sources::{DefaultSource, LowPriorityEnvSource, TomlFileSource};

#[cfg(feature = "routers")]
use {
	http_body_util::{BodyExt, Limited},
	reinhardt_commands::auto_register_router,
	reinhardt_http::Handler,
	reinhardt_urls::routers::get_router,
};

/// Settings bundle needed by the runserver command.
struct RunServerSettings {
	debug: bool,
	static_url: String,
	static_root: Option<PathBuf>,
	staticfiles_dirs: Vec<PathBuf>,
}

impl Default for RunServerSettings {
	fn default() -> Self {
		Self {
			debug: true,
			static_url: "/static/".to_string(),
			static_root: None,
			staticfiles_dirs: Vec::new(),
		}
	}
}

#[derive(Parser, Debug)]
#[command(name = "runserver")]
#[command(about = "Starts the development server", long_about = None)]
struct Args {
	/// Server address (default: 127.0.0.1:8000)
	#[arg(value_name = "ADDRESS", default_value = "127.0.0.1:8000")]
	address: String,

	/// Disable auto-reload
	#[arg(long)]
	noreload: bool,

	/// Watch delay in milliseconds for file change debouncing (default: 120)
	#[arg(long, default_value = "120")]
	watch_delay: u64,

	/// Disable threading
	#[arg(long)]
	nothreading: bool,

	/// Serve static files in production mode
	#[arg(long)]
	insecure: bool,

	/// Path to TLS certificate file (enables HTTPS)
	#[arg(long, value_name = "FILE")]
	cert: Option<PathBuf>,

	/// Path to TLS private key file (required with --cert)
	#[arg(long, value_name = "FILE")]
	key: Option<PathBuf>,

	/// Generate and use a self-signed certificate for development (enables HTTPS)
	#[arg(long)]
	self_signed: bool,

	/// Skip WASM builds at startup (also skips staleness checks; existing artifacts
	/// are served as-is regardless of source changes).
	#[arg(long)]
	no_wasm: bool,

	/// Reuse existing WASM artifacts in dist/ when they are up-to-date relative to
	/// sources (the prior default). Without this flag, WASM is always rebuilt.
	#[arg(long)]
	no_override_wasm: bool,

	/// DEPRECATED: rebuild is now the default. Use --no-override-wasm to opt out.
	#[arg(long)]
	force_wasm: bool,

	/// Skip collectstatic at startup
	#[arg(long)]
	no_collectstatic: bool,
}

/// Get MIME type based on file extension
fn get_mime_type(path: &Path) -> &'static str {
	match path.extension().and_then(|e| e.to_str()) {
		Some("js") => "application/javascript",
		Some("mjs") => "application/javascript",
		Some("css") => "text/css; charset=utf-8",
		Some("html") => "text/html; charset=utf-8",
		Some("htm") => "text/html; charset=utf-8",
		Some("json") => "application/json",
		Some("xml") => "application/xml",
		Some("png") => "image/png",
		Some("jpg") => "image/jpeg",
		Some("jpeg") => "image/jpeg",
		Some("gif") => "image/gif",
		Some("svg") => "image/svg+xml",
		Some("ico") => "image/x-icon",
		Some("woff") => "font/woff",
		Some("woff2") => "font/woff2",
		Some("ttf") => "font/ttf",
		Some("eot") => "application/vnd.ms-fontobject",
		Some("wasm") => "application/wasm",
		Some("mp4") => "video/mp4",
		Some("webm") => "video/webm",
		Some("mp3") => "audio/mpeg",
		Some("wav") => "audio/wav",
		Some("ogg") => "audio/ogg",
		Some("pdf") => "application/pdf",
		Some("zip") => "application/zip",
		Some("txt") => "text/plain; charset=utf-8",
		Some("md") => "text/markdown; charset=utf-8",
		_ => "application/octet-stream",
	}
}

/// Serve a static file
async fn serve_static_file(file_path: &Path) -> Result<Response<Full<Bytes>>, Infallible> {
	// Read file content
	match tokio::fs::read(file_path).await {
		Ok(content) => {
			let mime_type = get_mime_type(file_path);

			Ok(Response::builder()
				.status(StatusCode::OK)
				.header("Content-Type", mime_type)
				.header("Cache-Control", "no-cache")
				.body(Full::new(Bytes::from(content)))
				.unwrap())
		}
		Err(_) => Ok(Response::builder()
			.status(StatusCode::NOT_FOUND)
			.header("Content-Type", "text/plain")
			.body(Full::new(Bytes::from("File not found")))
			.unwrap()),
	}
}

/// Load settings from the settings directory
///
/// Settings are loaded from TOML files in the `settings/` directory:
/// - `base.toml` - Common settings across all environments
/// - `local.toml` / `production.toml` / `staging.toml` - Environment-specific settings
///
/// The environment is determined by the `REINHARDT_ENV` environment variable.
/// If no settings files exist, falls back to default settings.
fn load_settings() -> RunServerSettings {
	let profile_str = env::var("REINHARDT_ENV").unwrap_or_else(|_| "local".to_string());
	let profile = Profile::parse(&profile_str);

	let base_dir = env::current_dir().expect("Failed to get current directory");
	let settings_dir = base_dir.join("settings");

	// Check if settings directory exists
	if !settings_dir.exists() {
		eprintln!(
			"{}",
			"Warning: settings/ directory not found, using default settings".yellow()
		);
		return RunServerSettings::default();
	}

	// Build settings with priority: Default < LowPriorityEnv < base.toml < {profile}.toml
	let merged = SettingsBuilder::new()
		.profile(profile)
		.add_source(
			DefaultSource::new()
				// Core settings
				.with_value(
					"base_dir",
					serde_json::json!(base_dir.to_string_lossy().to_string()),
				)
				.with_value("debug", serde_json::json!(true))
				.with_value(
					"secret_key",
					serde_json::json!(generate_random_secret_key()),
				)
				.with_value("allowed_hosts", serde_json::json!([]))
				.with_value("installed_apps", serde_json::json!([]))
				.with_value("databases", serde_json::json!({}))
				.with_value("templates", serde_json::json!([]))
				// Static/Media files
				.with_value("static_url", serde_json::json!("/static/"))
				.with_value("static_root", serde_json::json!(null))
				.with_value("staticfiles_dirs", serde_json::json!([]))
				.with_value("media_url", serde_json::json!("/media/"))
				// Internationalization
				.with_value("language_code", serde_json::json!("en-us"))
				.with_value("time_zone", serde_json::json!("UTC"))
				.with_value("use_i18n", serde_json::json!(false))
				.with_value("use_tz", serde_json::json!(false))
				// Model settings
				.with_value(
					"default_auto_field",
					serde_json::json!("reinhardt.db.models.BigAutoField"),
				)
				// Security settings
				.with_value("secure_proxy_ssl_header", serde_json::json!(null))
				.with_value("secure_ssl_redirect", serde_json::json!(false))
				.with_value("secure_hsts_seconds", serde_json::json!(null))
				.with_value("secure_hsts_include_subdomains", serde_json::json!(false))
				.with_value("secure_hsts_preload", serde_json::json!(false))
				.with_value("session_cookie_secure", serde_json::json!(false))
				.with_value("csrf_cookie_secure", serde_json::json!(false))
				.with_value("append_slash", serde_json::json!(true))
				// Middleware
				.with_value("middleware", serde_json::json!([]))
				// URL configuration
				.with_value("root_urlconf", serde_json::json!(""))
				// Media files
				.with_value("media_root", serde_json::json!(null))
				// Admin/Manager contacts
				.with_value("admins", serde_json::json!([]))
				.with_value("managers", serde_json::json!([])),
		)
		.add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
		.add_source(TomlFileSource::new(settings_dir.join("base.toml")))
		.add_source(TomlFileSource::new(
			settings_dir.join(format!("{}.toml", profile_str)),
		))
		.build();

	match merged {
		Ok(merged_settings) => {
			let static_url: String = merged_settings.get_or("static_url", "/static/".to_string());
			let static_root: Option<PathBuf> = merged_settings.get("static_root").ok().flatten();
			let staticfiles_dirs: Vec<PathBuf> =
				merged_settings.get_or("staticfiles_dirs", Vec::new());
			match merged_settings.into_typed::<CoreSettings>() {
				Ok(core) => {
					println!(
						"{}",
						format!(
							"Loaded settings from settings/ directory (profile: {})",
							profile_str
						)
						.green()
					);
					RunServerSettings {
						debug: core.debug,
						static_url,
						static_root,
						staticfiles_dirs,
					}
				}
				Err(e) => {
					eprintln!(
						"{}",
						format!("Warning: Failed to parse settings: {}. Using defaults.", e)
							.yellow()
					);
					RunServerSettings::default()
				}
			}
		}
		Err(e) => {
			eprintln!(
				"{}",
				format!("Warning: Failed to build settings: {}. Using defaults.", e).yellow()
			);
			RunServerSettings::default()
		}
	}
}

#[cfg(feature = "routers")]
async fn dispatch_through_router(
	req: hyper::Request<hyper::body::Incoming>,
	remote_addr: std::net::SocketAddr,
) -> Option<hyper::Response<Full<Bytes>>> {
	const MAX_BODY: usize = 10 * 1024 * 1024; // 10 MiB
	let router = get_router()?;

	let (parts, body) = req.into_parts();
	let body_bytes = match Limited::new(body, MAX_BODY).collect().await {
		Ok(collected) => collected.to_bytes(),
		Err(_) => {
			return Some(
				hyper::Response::builder()
					.status(StatusCode::PAYLOAD_TOO_LARGE)
					.header("Content-Type", "text/plain; charset=utf-8")
					.body(Full::new(Bytes::from("Request body exceeds 10 MiB")))
					.expect("failed to build 413 response"),
			);
		}
	};
	let request = match reinhardt_http::Request::builder()
		.method(parts.method)
		.uri(parts.uri)
		.version(parts.version)
		.headers(parts.headers)
		.body(body_bytes)
		.remote_addr(remote_addr)
		.build()
	{
		Ok(r) => r,
		Err(e) => {
			eprintln!(
				"{}",
				format!("Warning: Failed to build router request: {}.", e).yellow()
			);
			return None;
		}
	};

	match router.handle(request).await {
		Ok(response) => convert_to_hyper_response(response),
		Err(e) => convert_to_hyper_response(reinhardt_http::Response::from(e)),
	}
}

#[cfg(feature = "routers")]
fn convert_to_hyper_response(
	response: reinhardt_http::Response,
) -> Option<hyper::Response<Full<Bytes>>> {
	if response.status == hyper::StatusCode::NOT_FOUND {
		return None;
	}
	let mut hyper_resp = hyper::Response::builder().status(response.status);
	for (key, value) in response.headers.iter() {
		hyper_resp = hyper_resp.header(key, value);
	}
	hyper_resp.body(Full::new(response.body)).ok()
}

/// Resolve a non-router request path against static files, SPA fallback, or the welcome page.
///
/// Kept separate from Hyper request dispatch so the static-serving behavior can be exercised
/// without opening a listener or constructing a streaming request body.
async fn respond_to_path(
	path: &str,
	settings: &RunServerSettings,
	spa_index: Option<&Path>,
) -> Result<Response<Full<Bytes>>, Infallible> {
	// Serve static files in debug mode from staticfiles_dirs
	if settings.debug && path.starts_with(&settings.static_url) {
		// Strip static_url prefix to get relative path
		let relative_path = match path.strip_prefix(&settings.static_url) {
			Some(p) => p,
			None => path,
		};
		let relative_path = relative_path.trim_start_matches('/');

		// If relative path is empty, serve the welcome page
		if relative_path.is_empty() {
			return serve_welcome_page();
		}

		// Find file in all staticfiles_dirs (in reverse order for override behavior)
		let mut found_files: Vec<PathBuf> = Vec::new();

		for dir in settings.staticfiles_dirs.iter().rev() {
			// Use safe_path_join to prevent path traversal attacks
			let file_path = match safe_path_join(dir, relative_path) {
				Ok(p) => p,
				Err(_) => continue,
			};
			if file_path.exists() && file_path.is_file() {
				found_files.push(file_path);
			}
		}

		// Check for conflicts (same file in multiple directories) - ERROR
		if found_files.len() > 1 {
			eprintln!(
				"❌ Error: Static file '{}' found in multiple directories:",
				relative_path
			);
			for path in &found_files {
				eprintln!("   - {}", path.display());
			}
			eprintln!("Please resolve the conflict by removing duplicate files.");
			return Ok(Response::builder()
				.status(StatusCode::INTERNAL_SERVER_ERROR)
				.header("Content-Type", "text/plain")
				.body(Full::new(Bytes::from(format!(
					"Internal Server Error: Static file conflict for '{}'. Check server logs.",
					relative_path
				))))
				.unwrap());
		}

		// Serve the found file
		if let Some(file_path) = found_files.first() {
			return serve_static_file(file_path).await;
		}

		// Also search STATIC_ROOT for already-collected files
		let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

		if let Some(ref root) = settings.static_root
			&& let Ok(file_path) = safe_path_join(root, relative_path)
			&& file_path.exists()
			&& file_path.is_file()
		{
			return serve_static_file(&file_path).await;
		}

		// Fallback: check <cwd>/staticfiles/
		let default_root = cwd.join("staticfiles");
		if let Ok(file_path) = safe_path_join(&default_root, relative_path)
			&& file_path.exists()
			&& file_path.is_file()
		{
			return serve_static_file(&file_path).await;
		}

		// File not found, return 404
		return Ok(Response::builder()
			.status(StatusCode::NOT_FOUND)
			.header("Content-Type", "text/plain")
			.body(Full::new(Bytes::from(format!(
				"Static file not found: {}",
				relative_path
			))))
			.unwrap());
	}

	// SPA fallback: serve index.html for non-static routes if available
	if let Some(index_path) = spa_index {
		return serve_static_file(index_path).await;
	}
	serve_welcome_page()
}

async fn handle_request(
	req: Request<Incoming>,
	settings: Arc<RunServerSettings>,
	spa_index: Option<Arc<PathBuf>>,
	_remote_addr: SocketAddr,
) -> Result<Response<Full<Bytes>>, Infallible> {
	let path = req.uri().path().to_string();

	// Route dispatch through registered ServerRouter
	#[cfg(feature = "routers")]
	{
		if let Some(response) = dispatch_through_router(req, _remote_addr).await {
			return Ok(response);
		}
	}

	respond_to_path(&path, &settings, spa_index.as_deref().map(PathBuf::as_path)).await
}

/// Serve the welcome page
fn serve_welcome_page() -> Result<Response<Full<Bytes>>, Infallible> {
	let component = WelcomePage::new(env!("CARGO_PKG_VERSION"));
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head(component.render());

	Ok(Response::builder()
		.status(StatusCode::OK)
		.header("Content-Type", "text/html; charset=utf-8")
		.body(Full::new(Bytes::from(html)))
		.unwrap())
}

/// Load TLS configuration from certificate and key files
fn load_tls_config(
	cert_path: &PathBuf,
	key_path: &PathBuf,
) -> Result<ServerConfig, Box<dyn std::error::Error>> {
	// Load certificate chain
	let cert_file = File::open(cert_path)?;
	let mut cert_reader = BufReader::new(cert_file);
	let cert_chain: Vec<_> = certs(&mut cert_reader).collect::<Result<_, _>>()?;

	// Load private key
	let key_file = File::open(key_path)?;
	let mut key_reader = BufReader::new(key_file);
	let private_key = private_key(&mut key_reader)?.ok_or("No private key found in key file")?;

	// Build TLS configuration
	let config = ServerConfig::builder()
		.with_no_client_auth()
		.with_single_cert(cert_chain, private_key)?;

	Ok(config)
}

/// Generate a self-signed certificate for development
fn generate_self_signed_cert() -> Result<
	(
		Vec<rustls::pki_types::CertificateDer<'static>>,
		rustls::pki_types::PrivateKeyDer<'static>,
	),
	Box<dyn std::error::Error>,
> {
	use rcgen::{CertificateParams, DistinguishedName, KeyPair};

	let mut params = CertificateParams::new(vec!["localhost".to_string()])?;
	let mut distinguished_name = DistinguishedName::new();
	distinguished_name.push(rcgen::DnType::CommonName, "Reinhardt Development Server");
	params.distinguished_name = distinguished_name;

	let key_pair = KeyPair::generate()?;
	let cert = params.self_signed(&key_pair)?;
	let cert_der = cert.der().to_vec();
	let key_der = key_pair.serialize_der();

	Ok((
		vec![rustls::pki_types::CertificateDer::from(cert_der)],
		rustls::pki_types::PrivateKeyDer::try_from(key_der)?,
	))
}

/// Generate a cryptographically random secret key for fallback use.
///
/// Produces a 50-character hex string (200 bits of entropy). This is used
/// as the default `SECRET_KEY` when no explicit key is configured, ensuring
/// that each process gets a unique key rather than a shared hardcoded value.
fn generate_random_secret_key() -> String {
	use rand::Rng;
	use std::fmt::Write;

	let mut rng = rand::rng();
	let bytes: [u8; 25] = rng.random();
	let mut hex_string = String::with_capacity(50);
	for b in bytes {
		let _ = write!(hex_string, "{:02x}", b);
	}
	hex_string
}

/// Build the admin WASM bundle from the reinhardt-admin crate.
///
/// Skips the build only when the existing `_bg.wasm` artifact is newer than every
/// tracked source file in the admin crate (mtime-based staleness check). See
/// [`build_pages_wasm`] for the rationale (issue #4127).
///
/// Returns `true` if the build succeeded or was skipped, `false` on failure.
#[cfg(feature = "admin")]
fn build_admin_wasm(force: bool) -> bool {
	// Determine workspace root from this binary's manifest dir
	let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	// Go up: src/bin → src → reinhardt-commands → crates → workspace root
	let workspace_root = manifest_dir
		.parent() // src/bin → src
		.and_then(|p| p.parent()) // src → reinhardt-commands
		.and_then(|p| p.parent()) // reinhardt-commands → crates
		.and_then(|p| p.parent()) // crates → workspace root
		.map(PathBuf::from)
		.unwrap_or_else(|| PathBuf::from("."));
	let admin_crate_dir = workspace_root.join("crates").join("reinhardt-admin");

	let artifact = admin_crate_dir
		.join("dist-admin")
		.join("reinhardt_admin_bg.wasm");
	if !force && !is_wasm_stale(&admin_crate_dir, &artifact) {
		println!(
			"{}",
			"Admin WASM: artifacts up to date, skipping build (--no-override-wasm)".dimmed()
		);
		return true;
	}

	let reason = if force {
		"forced rebuild"
	} else if artifact.exists() {
		"source changed since last build"
	} else {
		"no existing artifact"
	};
	println!("{}", format!("Building admin WASM ({})...", reason).cyan());
	let config = WasmBuildConfig::new(&admin_crate_dir)
		.output_dir("dist-admin")
		.target_name("reinhardt-admin");
	match WasmBuilder::new(config).build() {
		Ok(_) => {
			println!("{}", "Admin WASM build succeeded.".green());
			true
		}
		Err(e) => {
			eprintln!(
				"{}",
				format!("Warning: Admin WASM build failed: {}", e).yellow()
			);
			false
		}
	}
}

/// Build the pages WASM bundle from the current project (if it declares cdylib).
///
/// Staleness handling:
/// - If `dist/<crate>_bg.wasm` is missing or older than any tracked source file
///   (every `.rs` under `src/` plus `Cargo.toml`), the bundle is rebuilt.
/// - If the artifact is newer than every source file, the build is skipped.
/// - When `force` is `true`, the artifact is always rebuilt regardless of mtimes.
///
/// This guards against the dev-loop hazard where stale `dist/` content is served
/// after the developer edits Rust source — see issue #4127.
///
/// Returns `true` if the build succeeded or was skipped, `false` on failure or if the
/// current project is not a cdylib.
#[cfg(feature = "pages")]
fn build_pages_wasm(force: bool) -> bool {
	let cwd = match env::current_dir() {
		Ok(d) => d,
		Err(e) => {
			eprintln!(
				"{}",
				format!("Warning: Failed to get current directory: {}", e).yellow()
			);
			return false;
		}
	};
	let cargo_toml_path = cwd.join("Cargo.toml");

	// Only build if this project exports cdylib
	if !detect_cdylib_in_cargo_toml(&cargo_toml_path) {
		return false;
	}

	// Parse the crate name from Cargo.toml
	let crate_name = match std::fs::read_to_string(&cargo_toml_path) {
		Ok(content) => {
			let mut name = String::new();
			for line in content.lines() {
				let trimmed = line.trim();
				if trimmed.starts_with("name")
					&& trimmed.contains('=')
					&& let Some(val) = trimmed.split('=').nth(1)
				{
					name = val.trim().trim_matches('"').trim_matches('\'').to_string();
					break;
				}
			}
			if name.is_empty() {
				eprintln!(
					"{}",
					"Warning: Could not determine crate name from Cargo.toml".yellow()
				);
				return false;
			}
			name
		}
		Err(e) => {
			eprintln!(
				"{}",
				format!("Warning: Failed to read Cargo.toml: {}", e).yellow()
			);
			return false;
		}
	};

	let js_name = crate_name.replace('-', "_");
	let artifact = cwd.join("dist").join(format!("{}_bg.wasm", js_name));
	if !force && !is_wasm_stale(&cwd, &artifact) {
		println!(
			"{}",
			"Pages WASM: artifacts up to date, skipping build (--no-override-wasm)".dimmed()
		);
		return true;
	}

	let reason = if force {
		"forced rebuild"
	} else if artifact.exists() {
		"source changed since last build"
	} else {
		"no existing artifact"
	};
	println!(
		"{}",
		format!("Building pages WASM for {} ({})...", crate_name, reason).cyan()
	);
	// Resolve workspace root so wasm-bindgen finds the artifact in the
	// workspace-level target directory, not relative to the member crate CWD.
	let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let workspace_root = manifest_dir
		.parent()
		.and_then(|p| p.parent())
		.and_then(|p| p.parent())
		.and_then(|p| p.parent())
		.map(PathBuf::from)
		.unwrap_or_else(|| PathBuf::from("."));
	let config = WasmBuildConfig::new(".")
		.output_dir("dist")
		.target_dir(workspace_root.join("target"));
	match WasmBuilder::new(config).build() {
		Ok(_) => {
			println!("{}", "Pages WASM build succeeded.".green());
			true
		}
		Err(e) => {
			eprintln!(
				"{}",
				format!("Warning: Pages WASM build failed: {}", e).yellow()
			);
			false
		}
	}
}

/// Orchestrate WASM builds for all enabled targets.
///
/// `no_override_wasm` honours up-to-date `dist/` artifacts (the prior default);
/// without it, WASM is rebuilt unconditionally to avoid serving stale bundles.
/// `force_wasm_legacy` accepts the deprecated `--force-wasm` flag and emits a
/// warning; rebuild is otherwise the default.
fn build_wasm_targets(no_wasm: bool, no_override_wasm: bool, force_wasm_legacy: bool) {
	if no_wasm {
		println!("{}", "WASM builds skipped (--no-wasm)".dimmed());
		return;
	}

	if force_wasm_legacy {
		eprintln!(
			"{}",
			"Warning: --force-wasm is now the default behavior; this flag is deprecated. \
			 Use --no-override-wasm to opt out of rebuilds."
				.yellow()
		);
	}

	#[cfg(not(any(feature = "admin", feature = "pages")))]
	let _ = no_override_wasm;

	#[cfg(any(feature = "admin", feature = "pages"))]
	let force = !no_override_wasm;

	#[cfg(feature = "admin")]
	build_admin_wasm(force);

	#[cfg(feature = "pages")]
	build_pages_wasm(force);
}

/// Run collectstatic to copy all static files into STATIC_ROOT.
///
/// Returns `true` on success, `false` on failure.
fn run_collectstatic(settings: &RunServerSettings) -> bool {
	let cwd = match env::current_dir() {
		Ok(d) => d,
		Err(e) => {
			eprintln!(
				"{}",
				format!("Warning: Failed to get current directory: {}", e).yellow()
			);
			return false;
		}
	};

	// Determine STATIC_ROOT
	let static_root = match &settings.static_root {
		Some(root) => root.clone(),
		None => {
			let default_root = cwd.join("staticfiles");
			println!(
				"{}",
				format!(
					"STATIC_ROOT not configured, defaulting to {}",
					default_root.display()
				)
				.dimmed()
			);
			default_root
		}
	};

	let config = StaticFilesConfig {
		static_root: static_root.clone(),
		static_url: settings.static_url.clone(),
		staticfiles_dirs: settings.staticfiles_dirs.clone(),
		media_url: None,
	};

	let options = CollectStaticOptions {
		no_input: true,
		enable_hashing: true,
		verbosity: 1,
		..CollectStaticOptions::default()
	};

	let mut cmd = CollectStaticCommand::new(config, options);

	// If dist/index.html exists in cwd, set it as the index source
	let index_path = cwd.join("dist").join("index.html");
	if index_path.exists() {
		cmd.set_index_source(Some(index_path));
	}

	match cmd.execute() {
		Ok(stats) => {
			println!(
				"{}",
				format!(
					"collectstatic complete: {} copied, {} unmodified",
					stats.copied, stats.unmodified
				)
				.green()
			);
			true
		}
		Err(e) => {
			eprintln!(
				"{}",
				format!("Warning: collectstatic failed: {}", e).yellow()
			);
			false
		}
	}
}

/// Resolve the SPA index.html path for client-side routing fallback.
fn resolve_spa_index(settings: &RunServerSettings) -> Option<PathBuf> {
	let cwd = env::current_dir().ok()?;

	// Prefer configured STATIC_ROOT
	if let Some(ref root) = settings.static_root {
		let candidate = root.join("index.html");
		if candidate.exists() {
			return Some(candidate);
		}
	}

	// Fallback: <cwd>/staticfiles/index.html
	let candidate = cwd.join("staticfiles").join("index.html");
	if candidate.exists() {
		return Some(candidate);
	}

	None
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let args = Args::parse();

	// Validate TLS arguments
	if args.cert.is_some() && args.key.is_none() {
		return Err("--key is required when --cert is specified".into());
	}
	if args.key.is_some() && args.cert.is_none() {
		return Err("--cert is required when --key is specified".into());
	}
	if args.cert.is_some() && args.self_signed {
		return Err("Cannot use both --cert/--key and --self-signed".into());
	}

	// Phase 1: Build WASM targets
	build_wasm_targets(args.no_wasm, args.no_override_wasm, args.force_wasm);

	// Load settings at startup
	let settings = Arc::new(load_settings());

	// Phase 2: Run collectstatic
	if !args.no_collectstatic {
		run_collectstatic(&settings);
	} else {
		println!("{}", "collectstatic skipped (--no-collectstatic)".dimmed());
	}

	// Phase 3: Register HTTP routes from #[routes] inventory
	#[cfg(feature = "routers")]
	auto_register_router().await?;

	// Detect SPA index.html for client-side routing fallback
	let spa_index = resolve_spa_index(&settings).map(Arc::new);
	if spa_index.is_some() {
		println!(
			"{}",
			"SPA mode: index.html detected, enabling client-side routing fallback".green()
		);
	}

	// Display loaded settings info (debug mode only)
	if settings.debug {
		println!(
			"{}",
			format!(
				"Static files: URL={}, Directories={:?}",
				settings.static_url, settings.staticfiles_dirs
			)
			.dimmed()
		);
	}

	// Parse the address
	let addr: SocketAddr = args
		.address
		.parse()
		.map_err(|_| format!("Invalid address: {}", args.address))?;

	// Determine if HTTPS is enabled
	let use_https = args.cert.is_some() || args.self_signed;
	let scheme = if use_https { "https" } else { "http" };

	println!(
		"{}",
		format!("Starting development server at {}://{}", scheme, addr)
			.cyan()
			.bold()
	);

	if !args.noreload {
		println!("{}", "Auto-reload enabled".green());
	}

	if args.insecure {
		println!(
			"{}",
			"Running with --insecure: Static files will be served".yellow()
		);
	}

	// Load or generate TLS configuration if needed
	let tls_acceptor = if use_https {
		let tls_config = if args.self_signed {
			println!(
				"{}",
				"Using self-signed certificate for development".yellow()
			);
			let (certs, key) = generate_self_signed_cert()?;
			Arc::new(
				ServerConfig::builder()
					.with_no_client_auth()
					.with_single_cert(certs, key)?,
			)
		} else {
			let cert_path = args.cert.as_ref().unwrap();
			let key_path = args.key.as_ref().unwrap();
			println!(
				"{}",
				format!(
					"Loading TLS certificate from {:?} and key from {:?}",
					cert_path, key_path
				)
				.cyan()
			);
			Arc::new(load_tls_config(cert_path, key_path)?)
		};
		Some(TlsAcceptor::from(tls_config))
	} else {
		None
	};

	println!("{}", "Quit the server with CTRL-C".dimmed());
	println!();

	// Create TCP listener
	let listener = TcpListener::bind(addr).await?;

	println!("{}", format!("Listening on {}", addr).green().bold());

	// Accept connections in a loop
	loop {
		let (stream, peer_addr) = listener.accept().await?;

		if let Some(ref acceptor) = tls_acceptor {
			// HTTPS connection
			let acceptor = acceptor.clone();
			let settings_clone = Arc::clone(&settings);
			let spa_clone = spa_index.clone();
			tokio::task::spawn(async move {
				match acceptor.accept(stream).await {
					Ok(tls_stream) => {
						let io = TokioIo::new(tls_stream);
						if let Err(err) = http1::Builder::new()
							.serve_connection(
								io,
								service_fn(move |req| {
									let settings = Arc::clone(&settings_clone);
									let spa = spa_clone.clone();
									async move { handle_request(req, settings, spa, peer_addr).await }
								}),
							)
							.await
						{
							eprintln!("Error serving HTTPS connection: {:?}", err);
						}
					}
					Err(err) => {
						eprintln!("TLS handshake error: {:?}", err);
					}
				}
			});
		} else {
			// HTTP connection
			let settings_clone = Arc::clone(&settings);
			let spa_clone = spa_index.clone();
			let io = TokioIo::new(stream);
			tokio::task::spawn(async move {
				if let Err(err) = http1::Builder::new()
					.serve_connection(
						io,
						service_fn(move |req| {
							let settings = Arc::clone(&settings_clone);
							let spa = spa_clone.clone();
							async move { handle_request(req, settings, spa, peer_addr).await }
						}),
					)
					.await
				{
					eprintln!("Error serving HTTP connection: {:?}", err);
				}
			});
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use clap::Parser;
	use http_body_util::BodyExt;

	#[test]
	fn args_apply_documented_defaults_and_explicit_server_options() {
		// Act
		let defaults = Args::try_parse_from(["runserver"]).expect("default arguments parse");
		let configured = Args::try_parse_from([
			"runserver",
			"0.0.0.0:9443",
			"--noreload",
			"--watch-delay",
			"275",
			"--nothreading",
			"--insecure",
			"--cert",
			"server.pem",
			"--key",
			"server.key",
			"--no-wasm",
			"--no-override-wasm",
			"--force-wasm",
			"--no-collectstatic",
		])
		.expect("explicit server arguments parse");

		// Assert
		assert_eq!(defaults.address, "127.0.0.1:8000");
		assert_eq!(defaults.watch_delay, 120);
		assert!(!defaults.noreload && !defaults.self_signed && !defaults.no_wasm);
		assert_eq!(configured.address, "0.0.0.0:9443");
		assert_eq!(configured.watch_delay, 275);
		assert_eq!(configured.cert.as_deref(), Some(Path::new("server.pem")));
		assert_eq!(configured.key.as_deref(), Some(Path::new("server.key")));
		assert!(
			configured.noreload
				&& configured.nothreading
				&& configured.insecure
				&& configured.no_wasm
				&& configured.no_override_wasm
				&& configured.force_wasm
				&& configured.no_collectstatic
		);
	}

	#[test]
	fn static_file_extensions_produce_browser_content_types() {
		// Arrange
		let cases = [
			("app.js", "application/javascript"),
			("module.mjs", "application/javascript"),
			("site.css", "text/css; charset=utf-8"),
			("page.html", "text/html; charset=utf-8"),
			("feed.xml", "application/xml"),
			("photo.jpeg", "image/jpeg"),
			("vector.svg", "image/svg+xml"),
			("font.woff2", "font/woff2"),
			("bundle.wasm", "application/wasm"),
			("movie.webm", "video/webm"),
			("sound.ogg", "audio/ogg"),
			("guide.pdf", "application/pdf"),
			("notes.md", "text/markdown; charset=utf-8"),
			("unknown.bin", "application/octet-stream"),
			("no-extension", "application/octet-stream"),
			("UPPER.CSS", "application/octet-stream"),
		];

		for (file_name, expected) in cases {
			// Act
			let content_type = get_mime_type(Path::new(file_name));

			// Assert
			assert_eq!(content_type, expected, "unexpected type for {file_name}");
		}
	}

	#[tokio::test]
	async fn serve_static_file_returns_cacheable_content_and_not_found_response() {
		// Arrange
		let temp_dir = tempfile::TempDir::new().expect("create static fixture directory");
		let asset = temp_dir.path().join("app.css");
		tokio::fs::write(&asset, "body { color: green; }")
			.await
			.expect("write static fixture");

		// Act
		let served = serve_static_file(&asset)
			.await
			.expect("static response builds");
		let missing = serve_static_file(&temp_dir.path().join("missing.js"))
			.await
			.expect("not found response builds");
		let served_status = served.status();
		let served_content_type = served.headers()["Content-Type"].clone();
		let served_cache_control = served.headers()["Cache-Control"].clone();
		let missing_status = missing.status();
		let missing_content_type = missing.headers()["Content-Type"].clone();
		let served_body = served
			.into_body()
			.collect()
			.await
			.expect("served body is readable")
			.to_bytes();
		let missing_body = missing
			.into_body()
			.collect()
			.await
			.expect("not found body is readable")
			.to_bytes();

		// Assert
		assert_eq!(served_status, StatusCode::OK);
		assert_eq!(served_content_type, "text/css; charset=utf-8");
		assert_eq!(served_cache_control, "no-cache");
		assert_eq!(served_body, Bytes::from_static(b"body { color: green; }"));
		assert_eq!(missing_status, StatusCode::NOT_FOUND);
		assert_eq!(missing_content_type, "text/plain");
		assert_eq!(missing_body, Bytes::from_static(b"File not found"));
	}

	#[test]
	fn configured_spa_index_is_selected_for_client_side_route_fallback() {
		// Arrange
		let temp_dir = tempfile::TempDir::new().expect("create static root");
		let index = temp_dir.path().join("index.html");
		std::fs::write(&index, "<!doctype html><title>SPA</title>").expect("write SPA index");
		let settings = RunServerSettings {
			static_root: Some(temp_dir.path().to_path_buf()),
			..RunServerSettings::default()
		};

		// Act
		let resolved = resolve_spa_index(&settings);

		// Assert
		assert_eq!(resolved.as_deref(), Some(index.as_path()));
	}

	#[test]
	fn generated_fallback_secret_is_a_200_bit_hex_value() {
		// Act
		let secret = generate_random_secret_key();

		// Assert
		assert_eq!(secret.len(), 50);
		assert!(secret.bytes().all(|byte| byte.is_ascii_hexdigit()));
	}

	#[tokio::test]
	#[cfg(feature = "routers")]
	async fn router_response_conversion_preserves_success_and_skips_not_found() {
		// Act
		let converted = convert_to_hyper_response(
			reinhardt_http::Response::ok()
				.with_header("X-Route", "matched")
				.with_body("router body"),
		)
		.expect("handled route converts to Hyper response");
		let missing =
			convert_to_hyper_response(reinhardt_http::Response::new(StatusCode::NOT_FOUND));
		let (status, headers, body) = response_text(converted).await;

		// Assert
		assert_eq!(status, StatusCode::OK);
		assert_eq!(headers["X-Route"], "matched");
		assert_eq!(body, "router body");
		assert!(missing.is_none());
	}

	#[test]
	#[serial_test::serial(runserver_tls)]
	fn self_signed_tls_material_builds_a_server_configuration_and_rejects_invalid_pem() {
		// Arrange
		let _ = rustls::crypto::ring::default_provider().install_default();
		let temp_dir = tempfile::TempDir::new().expect("create TLS fixture directory");
		let cert_path = temp_dir.path().join("invalid-cert.pem");
		let key_path = temp_dir.path().join("invalid-key.pem");
		std::fs::write(&cert_path, "not a certificate").expect("write invalid certificate");
		std::fs::write(&key_path, "not a key").expect("write invalid key");

		// Act
		let (certificates, key) =
			generate_self_signed_cert().expect("generate development TLS material");
		let generated = ServerConfig::builder()
			.with_no_client_auth()
			.with_single_cert(certificates, key);
		let invalid = load_tls_config(&cert_path, &key_path);
		let missing = load_tls_config(&temp_dir.path().join("missing-cert.pem"), &key_path);

		// Assert
		assert!(generated.is_ok());
		assert!(invalid.is_err());
		assert!(missing.is_err());
		assert!(load_tls_config(&cert_path, &temp_dir.path().join("missing-key.pem")).is_err());
	}

	async fn response_text(
		response: Response<Full<Bytes>>,
	) -> (StatusCode, hyper::HeaderMap, String) {
		let (parts, body) = response.into_parts();
		let body = body
			.collect()
			.await
			.expect("response body is readable")
			.to_bytes();
		(
			parts.status,
			parts.headers,
			String::from_utf8(body.to_vec()).expect("response body is UTF-8 fixture text"),
		)
	}

	struct CurrentDirGuard {
		original: PathBuf,
	}

	impl CurrentDirGuard {
		fn enter(path: &Path) -> Self {
			let original = env::current_dir().expect("read working directory");
			env::set_current_dir(path).expect("enter temporary project directory");
			Self { original }
		}
	}

	impl Drop for CurrentDirGuard {
		fn drop(&mut self) {
			env::set_current_dir(&self.original).expect("restore working directory");
		}
	}

	struct EnvVarGuard {
		key: &'static str,
		original: Option<std::ffi::OsString>,
	}

	impl EnvVarGuard {
		fn capture(key: &'static str) -> Self {
			Self {
				key,
				original: env::var_os(key),
			}
		}
	}

	impl Drop for EnvVarGuard {
		fn drop(&mut self) {
			// SAFETY: this test serializes access to process-wide environment state.
			unsafe {
				match &self.original {
					Some(value) => env::set_var(self.key, value),
					None => env::remove_var(self.key),
				}
			}
		}
	}

	#[test]
	#[serial_test::serial(runserver_settings_environment)]
	fn load_settings_applies_project_static_configuration() {
		// Arrange
		let temp_dir = tempfile::TempDir::new().expect("create temporary project directory");
		let settings_dir = temp_dir.path().join("settings");
		std::fs::create_dir_all(&settings_dir).expect("create settings directory");
		std::fs::write(
			settings_dir.join("base.toml"),
			"debug = false\nstatic_url = \"/assets/\"\nstatic_root = \"public\"\nstaticfiles_dirs = [\"assets\"]\n",
		)
		.expect("write base settings");
		std::fs::write(settings_dir.join("local.toml"), "").expect("write local settings");
		let _environment = EnvVarGuard::capture("REINHARDT_ENV");
		unsafe { env::set_var("REINHARDT_ENV", "local") };
		let _cwd = CurrentDirGuard::enter(temp_dir.path());

		// Act
		let settings = load_settings();

		// Assert
		assert!(!settings.debug);
		assert_eq!(settings.static_url, "/assets/");
		assert_eq!(settings.static_root, Some(PathBuf::from("public")));
		assert_eq!(settings.staticfiles_dirs, vec![PathBuf::from("assets")]);
	}

	#[test]
	#[serial_test::serial(runserver_settings_environment)]
	fn run_collectstatic_copies_configured_assets_and_dist_index() {
		let project = tempfile::TempDir::new().expect("create temporary project");
		std::fs::create_dir_all(project.path().join("assets")).expect("create assets directory");
		std::fs::create_dir_all(project.path().join("dist")).expect("create dist directory");
		std::fs::write(
			project.path().join("assets/site.css"),
			"body { color: teal; }",
		)
		.expect("write static asset");
		std::fs::write(
			project.path().join("dist/index.html"),
			"<main>coverage app</main>",
		)
		.expect("write SPA index");
		let _cwd = CurrentDirGuard::enter(project.path());

		assert!(run_collectstatic(&RunServerSettings {
			static_url: "/assets/".to_string(),
			static_root: Some(PathBuf::from("public")),
			staticfiles_dirs: vec![PathBuf::from("assets")],
			..RunServerSettings::default()
		}));
		let manifest: serde_json::Value = serde_json::from_str(
			&std::fs::read_to_string("public/manifest.json").expect("read asset manifest"),
		)
		.expect("parse asset manifest");
		let emitted_asset = manifest["paths"]["site.css"]
			.as_str()
			.expect("hashed asset entry");
		assert_eq!(
			std::fs::read_to_string(format!("public/{emitted_asset}")).expect("read copied asset"),
			"body { color: teal; }"
		);
		assert_eq!(
			std::fs::read_to_string("public/index.html").expect("read copied index"),
			"<main>coverage app</main>"
		);
	}

	#[test]
	#[serial_test::serial(runserver_settings_environment)]
	fn runserver_fallbacks_use_defaults_without_building_wasm() {
		let project = tempfile::TempDir::new().expect("create temporary project");
		let _environment = EnvVarGuard::capture("REINHARDT_ENV");
		unsafe { env::set_var("REINHARDT_ENV", "local") };
		let _cwd = CurrentDirGuard::enter(project.path());

		let missing = load_settings();
		assert!(missing.debug);
		assert_eq!(missing.static_url, "/static/");
		assert_eq!(missing.static_root, None);
		assert!(missing.staticfiles_dirs.is_empty());
		std::fs::create_dir("settings").expect("create settings directory");
		std::fs::write("settings/base.toml", "[invalid").expect("write malformed settings");
		std::fs::write("settings/local.toml", "").expect("write local settings");
		let malformed = load_settings();
		assert!(malformed.debug);
		assert_eq!(malformed.static_url, "/static/");
		assert_eq!(malformed.static_root, None);
		assert!(malformed.staticfiles_dirs.is_empty());
		build_wasm_targets(true, false, false);
		assert!(resolve_spa_index(&missing).is_none());
		assert!(run_collectstatic(&missing));
		assert!(Path::new("staticfiles/manifest.json").is_file());
		std::fs::write("blocked", "not a directory").expect("write static-root blocker");
		assert!(!run_collectstatic(&RunServerSettings {
			static_root: Some(PathBuf::from("blocked/child")),
			..missing
		}));
	}

	#[tokio::test]
	async fn path_resolution_serves_static_directory_assets_and_rejects_conflicts() {
		// Arrange
		let temp_dir = tempfile::TempDir::new().expect("create static fixture directory");
		let first = temp_dir.path().join("first");
		let second = temp_dir.path().join("second");
		std::fs::create_dir_all(&first).expect("create first static directory");
		std::fs::create_dir_all(&second).expect("create second static directory");
		std::fs::write(first.join("site.css"), "from-first").expect("write first asset");
		let settings = RunServerSettings {
			staticfiles_dirs: vec![first.clone()],
			..RunServerSettings::default()
		};

		// Act
		let served = respond_to_path("/static/site.css", &settings, None)
			.await
			.expect("static response builds");
		std::fs::write(second.join("site.css"), "from-second").expect("write conflicting asset");
		let conflicting_settings = RunServerSettings {
			staticfiles_dirs: vec![first, second],
			..RunServerSettings::default()
		};
		let conflict = respond_to_path("/static/site.css", &conflicting_settings, None)
			.await
			.expect("conflict response builds");
		let (served_status, served_headers, served_body) = response_text(served).await;
		let (conflict_status, conflict_headers, conflict_body) = response_text(conflict).await;

		// Assert
		assert_eq!(served_status, StatusCode::OK);
		assert_eq!(served_headers["Content-Type"], "text/css; charset=utf-8");
		assert_eq!(served_body, "from-first");
		assert_eq!(conflict_status, StatusCode::INTERNAL_SERVER_ERROR);
		assert_eq!(conflict_headers["Content-Type"], "text/plain");
		assert_eq!(
			conflict_body,
			"Internal Server Error: Static file conflict for 'site.css'. Check server logs."
		);
	}

	#[tokio::test]
	async fn path_resolution_uses_collected_assets_then_spa_fallback() {
		// Arrange
		let temp_dir = tempfile::TempDir::new().expect("create collected asset root");
		let root = temp_dir.path().join("collected");
		std::fs::create_dir_all(&root).expect("create collected root");
		std::fs::write(root.join("bundle.js"), "console.log('collected');")
			.expect("write collected asset");
		let spa_index = root.join("index.html");
		std::fs::write(&spa_index, "<main>single-page app</main>").expect("write SPA index");
		let settings = RunServerSettings {
			static_root: Some(root),
			..RunServerSettings::default()
		};

		// Act
		let collected = respond_to_path("/static/bundle.js", &settings, None)
			.await
			.expect("collected asset response builds");
		let spa = respond_to_path("/dashboard", &settings, Some(&spa_index))
			.await
			.expect("SPA response builds");
		let (collected_status, collected_headers, collected_body) = response_text(collected).await;
		let (spa_status, spa_headers, spa_body) = response_text(spa).await;

		// Assert
		assert_eq!(collected_status, StatusCode::OK);
		assert_eq!(collected_headers["Content-Type"], "application/javascript");
		assert_eq!(collected_body, "console.log('collected');");
		assert_eq!(spa_status, StatusCode::OK);
		assert_eq!(spa_headers["Content-Type"], "text/html; charset=utf-8");
		assert_eq!(spa_body, "<main>single-page app</main>");
	}

	#[tokio::test]
	async fn path_resolution_reports_missing_and_traversal_static_assets_without_exposing_files() {
		// Arrange
		let temp_dir = tempfile::TempDir::new().expect("create isolated static root");
		let static_root = temp_dir.path().join("public");
		std::fs::create_dir_all(&static_root).expect("create static root");
		std::fs::write(
			temp_dir.path().join("secret.txt"),
			"coverage sentinel secret",
		)
		.expect("write protected sibling sentinel");
		let settings = RunServerSettings {
			static_root: Some(static_root),
			..RunServerSettings::default()
		};

		// Act
		let missing = respond_to_path("/static/missing-coverage-asset.css", &settings, None)
			.await
			.expect("missing response builds");
		let traversal = respond_to_path("/static/../secret.txt", &settings, None)
			.await
			.expect("traversal response builds");
		let (missing_status, _, missing_body) = response_text(missing).await;
		let (traversal_status, _, traversal_body) = response_text(traversal).await;

		// Assert
		assert_eq!(missing_status, StatusCode::NOT_FOUND);
		assert_eq!(
			missing_body,
			"Static file not found: missing-coverage-asset.css"
		);
		assert_eq!(traversal_status, StatusCode::NOT_FOUND);
		assert_eq!(traversal_body, "Static file not found: ../secret.txt");
		assert_ne!(traversal_body, "coverage sentinel secret");
	}

	#[tokio::test]
	async fn path_resolution_serves_welcome_page_for_static_root_requests() {
		// Arrange
		let settings = RunServerSettings::default();

		// Act
		let welcome = respond_to_path("/static/", &settings, None)
			.await
			.expect("welcome response builds");
		let (status, headers, body) = response_text(welcome).await;
		let component = WelcomePage::new(env!("CARGO_PKG_VERSION"));
		let mut renderer = SsrRenderer::new();
		let expected = renderer.render_page_with_view_head(component.render());

		// Assert
		assert_eq!(status, StatusCode::OK);
		assert_eq!(headers["Content-Type"], "text/html; charset=utf-8");
		assert_eq!(body, expected);
	}
}
