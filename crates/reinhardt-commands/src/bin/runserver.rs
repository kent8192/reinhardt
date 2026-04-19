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
use reinhardt_commands::{WasmBuildConfig, WasmBuilder, detect_cdylib_in_cargo_toml};
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

use reinhardt_conf::Settings;
use reinhardt_conf::settings::builder::SettingsBuilder;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::sources::{DefaultSource, LowPriorityEnvSource, TomlFileSource};

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

	/// Watch delay in milliseconds for file change debouncing (default: 500)
	#[arg(long, default_value = "500")]
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

	/// Skip WASM builds at startup
	#[arg(long)]
	no_wasm: bool,

	/// Force rebuild WASM even if artifacts exist
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
fn load_settings() -> Settings {
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
		return Settings::default();
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
		Ok(merged_settings) => match merged_settings.into_typed::<Settings>() {
			Ok(settings) => {
				println!(
					"{}",
					format!(
						"Loaded settings from settings/ directory (profile: {})",
						profile_str
					)
					.green()
				);
				settings
			}
			Err(e) => {
				eprintln!(
					"{}",
					format!("Warning: Failed to parse settings: {}. Using defaults.", e).yellow()
				);
				Settings::default()
			}
		},
		Err(e) => {
			eprintln!(
				"{}",
				format!("Warning: Failed to build settings: {}. Using defaults.", e).yellow()
			);
			Settings::default()
		}
	}
}

async fn handle_request(
	req: Request<Incoming>,
	settings: Arc<Settings>,
	spa_index: Option<Arc<PathBuf>>,
) -> Result<Response<Full<Bytes>>, Infallible> {
	let path = req.uri().path();

	// Serve static files in debug mode from staticfiles_dirs
	if settings.core.debug && path.starts_with(&settings.static_url) {
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
	if let Some(ref index_path) = spa_index {
		return serve_static_file(index_path).await;
	}
	serve_welcome_page()
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
		.join("reinhardt_admin.js");
	if artifact.exists() && !force {
		println!(
			"{}",
			"Admin WASM: artifacts exist, skipping build (use --force-wasm to rebuild)".dimmed()
		);
		return true;
	}

	println!("{}", "Building admin WASM...".cyan());
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
	let artifact = cwd.join("dist").join(format!("{}.js", js_name));
	if artifact.exists() && !force {
		println!(
			"{}",
			"Pages WASM: artifacts exist, skipping build (use --force-wasm to rebuild)".dimmed()
		);
		return true;
	}

	println!(
		"{}",
		format!("Building pages WASM for {}...", crate_name).cyan()
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
fn build_wasm_targets(no_wasm: bool, force_wasm: bool) {
	if no_wasm {
		println!("{}", "WASM builds skipped (--no-wasm)".dimmed());
		return;
	}

	#[cfg(feature = "admin")]
	build_admin_wasm(force_wasm);

	#[cfg(feature = "pages")]
	build_pages_wasm(force_wasm);
}

/// Run collectstatic to copy all static files into STATIC_ROOT.
///
/// Returns `true` on success, `false` on failure.
fn run_collectstatic(settings: &Settings) -> bool {
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
fn resolve_spa_index(settings: &Settings) -> Option<PathBuf> {
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
	build_wasm_targets(args.no_wasm, args.force_wasm);

	// Load settings at startup
	let settings = Arc::new(load_settings());

	// Phase 2: Run collectstatic
	if !args.no_collectstatic {
		run_collectstatic(&settings);
	} else {
		println!("{}", "collectstatic skipped (--no-collectstatic)".dimmed());
	}

	// Detect SPA index.html for client-side routing fallback
	let spa_index = resolve_spa_index(&settings).map(Arc::new);
	if spa_index.is_some() {
		println!(
			"{}",
			"SPA mode: index.html detected, enabling client-side routing fallback".green()
		);
	}

	// Display loaded settings info (debug mode only)
	if settings.core.debug {
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
		let (stream, _) = listener.accept().await?;

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
									async move { handle_request(req, settings, spa).await }
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
							async move { handle_request(req, settings, spa).await }
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
