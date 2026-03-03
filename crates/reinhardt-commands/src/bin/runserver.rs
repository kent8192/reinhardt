//! runserver CLI command
//!
//! Starts the development server.

use clap::Parser;
use colored::Colorize;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode, body::Incoming};
use hyper_util::rt::TokioIo;
use reinhardt_commands::WelcomePage;
use reinhardt_pages::component::Component;
use reinhardt_pages::ssr::SsrRenderer;
use reinhardt_utils::safe_path_join;
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
) -> Result<Response<Full<Bytes>>, Infallible> {
	let path = req.uri().path();

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

	// Fall through to welcome page
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

	// Load settings at startup
	let settings = Arc::new(load_settings());

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
		let (stream, _) = listener.accept().await?;

		if let Some(ref acceptor) = tls_acceptor {
			// HTTPS connection
			let acceptor = acceptor.clone();
			let settings_clone = Arc::clone(&settings);
			tokio::task::spawn(async move {
				match acceptor.accept(stream).await {
					Ok(tls_stream) => {
						let io = TokioIo::new(tls_stream);
						if let Err(err) = http1::Builder::new()
							.serve_connection(
								io,
								service_fn(move |req| {
									let settings = Arc::clone(&settings_clone);
									async move { handle_request(req, settings).await }
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
			let io = TokioIo::new(stream);
			tokio::task::spawn(async move {
				if let Err(err) = http1::Builder::new()
					.serve_connection(
						io,
						service_fn(move |req| {
							let settings = Arc::clone(&settings_clone);
							async move { handle_request(req, settings).await }
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
