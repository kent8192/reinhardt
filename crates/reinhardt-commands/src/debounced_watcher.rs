//! Debounced file-system watcher for hot-reload.
//!
//! This module owns the `notify::RecommendedWatcher` and the per-event dispatch
//! loop that fans out to the WASM and server rebuild pipelines. The outer loop
//! is intentionally resilient: pipeline failures are logged but never bubble
//! out as `Err`, so a fix-and-save retry path always exists (spec §6 OL-1).
//!
//! The public surface is `pub(crate)` because callers live inside the same
//! crate (`builtin::Runserver::run_with_autoreload`).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use notify::Event;
#[cfg(feature = "pages")]
use tokio::sync::broadcast;
use tokio::sync::mpsc::Receiver;
use tokio::sync::oneshot;
use tokio::time::{Instant, sleep, timeout_at};

use crate::CommandContext;
use crate::source_roots::SourceRoots;
#[cfg(feature = "pages")]
use crate::{
	DispatchOutcome, TemplateBuildArtifact, TemplateHotReloadCoordinator,
	template_manifest::collect_client_baseline,
};

/// Default time window over which bursts of events are coalesced into a single
/// rebuild trigger.
pub const DEBOUNCE_WINDOW: Duration = Duration::from_millis(120);

/// Decide whether a `notify::Event` should trigger a rebuild.
///
/// The accept rules are intentionally narrow:
/// * Event kind must be `Modify`, `Create`, or `Remove`.
/// * At least one path must end in `.rs` or `.toml`, or have the exact
///   file name `Cargo.lock` (matched via `Path::file_name`, so unrelated
///   `.lock` files and `Cargo.lock.bak` do not slip through).
/// * Paths inside `target/` or `.git/`, and editor sidecar files
///   (`~`, `.swp`, `.tmp`), are rejected.
pub fn is_relevant_change(event: &Event) -> bool {
	use notify::EventKind;

	if !matches!(
		event.kind,
		EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
	) {
		return false;
	}
	event.paths.iter().any(|p| {
		let path_str = p.to_string_lossy();
		// `Path::file_name` (not suffix matching) so that `Cargo.lock.bak`
		// and unrelated `.lock` files do not slip through. See issue #4214.
		let is_cargo_lock = p.file_name() == Some(std::ffi::OsStr::new("Cargo.lock"));
		!path_str.contains("/target/")
			&& !path_str.contains("/.git/")
			&& !path_str.ends_with('~')
			&& !path_str.ends_with(".swp")
			&& !path_str.ends_with(".tmp")
			&& (path_str.ends_with(".rs") || path_str.ends_with(".toml") || is_cargo_lock)
	})
}

/// Block until a relevant event arrives, then keep collecting any further
/// events that arrive within `window` of *now*.
///
/// Returns the deduplicated, sorted set of paths from all coalesced events,
/// or `None` if the channel closes before any relevant event arrives.
pub async fn debounce_next(rx: &mut Receiver<Event>, window: Duration) -> Option<Vec<PathBuf>> {
	// Phase 1: wait for the first relevant event (drop noise silently).
	let first = loop {
		match rx.recv().await {
			Some(ev) if is_relevant_change(&ev) => break ev,
			Some(_) => continue,
			None => return None,
		}
	};

	let mut paths: BTreeSet<PathBuf> = BTreeSet::new();
	for p in first.paths {
		paths.insert(p);
	}

	// Phase 2: collect any further events arriving before the absolute
	// deadline. Using an absolute deadline (rather than a fresh per-event
	// timeout) bounds total wait time even under sustained event bursts.
	let deadline = Instant::now() + window;
	loop {
		match timeout_at(deadline, rx.recv()).await {
			Ok(Some(ev)) => {
				if is_relevant_change(&ev) {
					for p in ev.paths {
						paths.insert(p);
					}
				}
			}
			Ok(None) => break, // channel closed
			Err(_) => break,   // deadline reached
		}
	}

	Some(paths.into_iter().collect())
}

/// Rebuild pipelines selected for a debounced change batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RebuildTargets {
	/// Rebuild and respawn the native server binary.
	pub server: bool,
	/// Rebuild the Pages WASM bundle.
	pub wasm: bool,
}

impl RebuildTargets {
	fn has_work(self) -> bool {
		self.server || self.wasm
	}
}

/// Configuration for `run_watcher`.
pub struct WatcherConfig {
	/// Project root used to derive stable, project-relative template keys.
	#[cfg(feature = "pages")]
	pub project_root: PathBuf,
	/// Bin name passed to `cargo build --bin`.
	pub bin_name: String,
	/// Advertised runserver address that must be reachable after restart.
	pub address: String,
	/// Source directories and manifest files to subscribe to.
	pub roots: SourceRoots,
	/// Time window used to coalesce bursty filesystem events.
	pub debounce_window: Duration,
	/// HTTP address used by the child server. When set, browser reloads that
	/// depend on a server respawn wait briefly for this address to accept TCP.
	pub server_address: Option<String>,
	/// When `true`, suppress the WASM rebuild pipeline.
	pub no_wasm_rebuild: bool,
	/// When `true`, the project has the pages feature enabled.
	#[cfg(feature = "pages")]
	pub pages_enabled: bool,
	/// Browser HMR channel used to reload connected Pages clients after a
	/// successful rebuild. `None` keeps the watcher in compile-only mode.
	#[cfg(feature = "pages")]
	pub hmr_tx: Option<broadcast::Sender<String>>,
	/// HMR server used for retained patch replay and client acknowledgements.
	#[cfg(feature = "pages")]
	pub hmr_server: Option<reinhardt_pages::hmr::HmrServer>,
	/// Last-good component style compiler state processed before rebuild classification.
	#[cfg(feature = "pages")]
	pub component_styles: Option<std::sync::Arc<std::sync::Mutex<crate::ComponentStyleState>>>,
}

/// Cargo configuration used by native hot-reload rebuilds.
#[derive(Clone, Copy)]
pub(crate) struct ServerRebuildContext<'a> {
	package: Option<&'a str>,
	features: &'a [String],
	all_features: bool,
}

impl<'a> ServerRebuildContext<'a> {
	pub(crate) fn new(
		package: Option<&'a str>,
		features: &'a [String],
		all_features: bool,
	) -> Self {
		Self {
			package,
			features,
			all_features,
		}
	}

	pub(crate) fn for_native_server() -> Self {
		Self::default()
	}
}

impl Default for ServerRebuildContext<'_> {
	fn default() -> Self {
		Self::new(None, &[], false)
	}
}

/// Shared state required to rebuild one debounced path batch.
struct WatcherRebuildContext<'a, 'b> {
	command_context: &'a CommandContext,
	config: &'a WatcherConfig,
	server_rebuild_context: ServerRebuildContext<'b>,
}

/// Select rebuild pipelines for a debounced path batch.
///
/// The classifier is deliberately conservative. It only suppresses a
/// pipeline for generated Pages paths whose ownership is stable:
/// `src/client.rs`, `src/client/**`, and `src/apps/*/client/**` are WASM-only;
/// `src/bin/**` and server process configuration are native-only. Shared code,
/// manifests, and lockfiles still rebuild both sides.
pub fn rebuild_targets_for_paths(paths: &[PathBuf], config: &WatcherConfig) -> RebuildTargets {
	#[cfg(feature = "pages")]
	let pages_enabled = config.pages_enabled;
	#[cfg(not(feature = "pages"))]
	let pages_enabled = false;

	if !pages_enabled {
		return RebuildTargets {
			server: !paths.is_empty(),
			wasm: false,
		};
	}

	let wasm_enabled = !config.no_wasm_rebuild;
	let mut targets = RebuildTargets {
		server: false,
		wasm: false,
	};

	for path in paths {
		match classify_pages_path(path) {
			PagesPathClass::WasmOnly => {
				targets.wasm |= wasm_enabled;
			}
			PagesPathClass::ServerOnly => {
				targets.server = true;
			}
			PagesPathClass::Both => {
				targets.server = true;
				targets.wasm |= wasm_enabled;
			}
		}
	}

	targets
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PagesPathClass {
	ServerOnly,
	WasmOnly,
	Both,
}

fn classify_pages_path(path: &std::path::Path) -> PagesPathClass {
	let normalized = normalized_path(path);
	let wrapped = format!("/{normalized}");

	if wrapped.ends_with("/Cargo.toml") || wrapped.ends_with("/Cargo.lock") {
		return PagesPathClass::Both;
	}

	if wrapped.ends_with("/src/client.rs")
		|| wrapped.contains("/src/client/")
		|| (wrapped.contains("/src/apps/") && wrapped.contains("/client/"))
	{
		return PagesPathClass::WasmOnly;
	}

	if wrapped.contains("/src/bin/")
		|| wrapped.ends_with("/src/config/apps.rs")
		|| wrapped.ends_with("/src/config/settings.rs")
	{
		return PagesPathClass::ServerOnly;
	}

	if wrapped.ends_with(".toml") {
		return PagesPathClass::ServerOnly;
	}

	PagesPathClass::Both
}

fn normalized_path(path: &std::path::Path) -> String {
	path.to_string_lossy().replace('\\', "/")
}

fn paths_change_cargo_metadata(paths: &[PathBuf]) -> bool {
	paths.iter().any(|path| {
		path.file_name()
			.is_some_and(|name| name == "Cargo.toml" || name == "Cargo.lock")
	})
}

#[cfg(feature = "pages")]
fn can_attempt_template_patch(paths: &[PathBuf], config: &WatcherConfig) -> bool {
	if !config.pages_enabled || config.no_wasm_rebuild || paths_change_cargo_metadata(paths) {
		return false;
	}
	let targets = rebuild_targets_for_paths(paths, config);
	targets.wasm && !targets.server
}

#[cfg(feature = "pages")]
fn wasm_rebuild_succeeded(outcome: &crate::wasm_rebuild_pipeline::WasmRebuildOutcome) -> bool {
	matches!(
		outcome,
		crate::wasm_rebuild_pipeline::WasmRebuildOutcome::Ok { .. }
			| crate::wasm_rebuild_pipeline::WasmRebuildOutcome::Skipped
	)
}

fn server_rebuild_succeeded(
	outcome: &crate::server_rebuild_pipeline::ServerRebuildOutcome,
) -> bool {
	matches!(
		outcome,
		crate::server_rebuild_pipeline::ServerRebuildOutcome::Ok { .. }
	)
}

async fn wait_for_server_ready(address: Option<&str>) -> bool {
	let Some(address) = address else {
		return true;
	};

	let deadline = Instant::now() + Duration::from_secs(2);
	loop {
		if tokio::net::TcpStream::connect(address).await.is_ok() {
			return true;
		}
		if Instant::now() >= deadline {
			return false;
		}
		sleep(Duration::from_millis(50)).await;
	}
}

#[cfg(feature = "pages")]
async fn next_hmr_client_event(receiver: &mut Option<broadcast::Receiver<String>>) -> String {
	loop {
		let Some(receiver) = receiver.as_mut() else {
			return std::future::pending::<String>().await;
		};
		match receiver.recv().await {
			Ok(event) => return event,
			Err(broadcast::error::RecvError::Lagged(_)) => continue,
			Err(broadcast::error::RecvError::Closed) => {
				return std::future::pending::<String>().await;
			}
		}
	}
}

#[cfg(not(feature = "pages"))]
async fn next_hmr_client_event(_: &mut ()) -> String {
	std::future::pending::<String>().await
}

#[cfg(feature = "pages")]
fn notify_browser_reload(hmr_tx: Option<&broadcast::Sender<String>>, reason: &str) -> bool {
	let Some(tx) = hmr_tx else {
		return false;
	};
	let msg = reinhardt_pages::hmr::HmrMessage::FullReload {
		reason: reason.to_string(),
	};
	if let Ok(json) = msg.to_json() {
		return tx.send(json).is_ok();
	}
	false
}

#[cfg(feature = "pages")]
async fn notify_server_reload_if_ready(
	server_outcome: &crate::server_rebuild_pipeline::ServerRebuildOutcome,
	config: &WatcherConfig,
) {
	if server_rebuild_succeeded(server_outcome)
		&& wait_for_server_ready(config.server_address.as_deref()).await
	{
		notify_browser_reload(
			config.hmr_tx.as_ref(),
			"Server rebuild completed successfully",
		);
	}
}

#[cfg(feature = "pages")]
fn commit_pending_component_styles(
	component_styles: Option<&std::sync::Arc<std::sync::Mutex<crate::ComponentStyleState>>>,
	wasm_rebuilt: bool,
) -> bool {
	if !wasm_rebuilt {
		return false;
	}
	let Some(state) = component_styles else {
		return true;
	};
	match state.lock() {
		Ok(mut state) => match state.commit_pending() {
			Ok(_) => true,
			Err(error) => {
				eprintln!("component stylesheet replacement failed after rebuild: {error}");
				false
			}
		},
		Err(_) => {
			eprintln!("component stylesheet state lock was poisoned after rebuild");
			false
		}
	}
}

/// Dispatch one debounced path batch through the selected rebuild pipelines.
///
/// This is split out from [`run_watcher`] so tests can validate the
/// rebuild-to-HMR contract deterministically without depending on OS file
/// notification timing.
pub async fn run_rebuild_for_paths(
	ctx: &CommandContext,
	config: &WatcherConfig,
	paths: Vec<PathBuf>,
	current_child: &mut tokio::process::Child,
	respawn: &(impl Fn() -> std::io::Result<tokio::process::Child> + Send + Sync),
) {
	run_rebuild_for_paths_for_package(
		WatcherRebuildContext {
			command_context: ctx,
			config,
			server_rebuild_context: ServerRebuildContext::default(),
		},
		paths,
		current_child,
		respawn,
		#[cfg(feature = "pages")]
		None,
		#[cfg(feature = "pages")]
		None,
	)
	.await;
}

async fn run_rebuild_for_paths_for_package(
	watcher_rebuild_context: WatcherRebuildContext<'_, '_>,
	paths: Vec<PathBuf>,
	current_child: &mut tokio::process::Child,
	respawn: &(impl Fn() -> std::io::Result<tokio::process::Child> + Send + Sync),
	#[cfg(feature = "pages")] mut template_coordinator: Option<&mut TemplateHotReloadCoordinator>,
	#[cfg(feature = "pages")] template_fallback_generation: Option<
		reinhardt_pages::hmr::PatchGeneration,
	>,
) {
	let WatcherRebuildContext {
		command_context: ctx,
		config,
		server_rebuild_context: rebuild_context,
	} = watcher_rebuild_context;

	ctx.info(&format!(
		"[hot-reload] change detected ({} path(s))",
		paths.len()
	));
	let targets = rebuild_targets_for_paths(&paths, config);
	#[cfg(feature = "pages")]
	let metadata_changed = paths_change_cargo_metadata(&paths);
	#[cfg(feature = "pages")]
	let (style_stage, component_style_paths_only) = if let Some(state) = &config.component_styles {
		match state.lock() {
			Ok(mut state) => {
				let component_style_paths_only =
					paths.iter().all(|path| state.tracks_source_path(path));
				(state.refresh(metadata_changed), component_style_paths_only)
			}
			Err(_) => (crate::ComponentStyleStageResult::Failed, false),
		}
	} else {
		(crate::ComponentStyleStageResult::Unchanged, false)
	};
	#[cfg(feature = "pages")]
	let css_only = style_stage == crate::ComponentStyleStageResult::CssOnly && !metadata_changed;
	#[cfg(feature = "pages")]
	if css_only {
		notify_component_stylesheet_update(config.hmr_tx.as_ref());
	}
	#[cfg(feature = "pages")]
	if can_short_circuit_css_only(style_stage, metadata_changed, component_style_paths_only) {
		ctx.info("[hot-reload] component stylesheet updated without rebuilding");
		return;
	}
	#[cfg(feature = "pages")]
	if css_only {
		ctx.info("[hot-reload] component stylesheet updated; rebuilding other changed targets");
	}
	if !targets.has_work() {
		ctx.info("[hot-reload] no rebuild target matched; waiting for next change");
		return;
	}
	let wasm_fut = async {
		#[cfg(feature = "pages")]
		{
			if targets.wasm {
				let outcome = crate::wasm_rebuild_pipeline::WasmRebuildPipeline::run(ctx).await;
				if let Some(line) =
					crate::wasm_rebuild_pipeline::WasmRebuildPipeline::format_log_line(&outcome)
				{
					eprintln!("{}", line);
					if matches!(
						outcome,
						crate::wasm_rebuild_pipeline::WasmRebuildOutcome::Failed { .. }
					) {
						eprintln!("[hot-reload] watching for next change...");
					}
				}
				return wasm_rebuild_succeeded(&outcome);
			}
		}
		true
	};

	if targets.server && targets.wasm {
		// Spec §4: run wasm + server pipelines in parallel. They touch
		// disjoint cargo target directories (`wasm32-unknown-unknown` vs
		// `debug`) and the wasm pipeline does not interact with the running
		// child process, so concurrent execution is safe.
		let server_fut =
			crate::server_rebuild_pipeline::ServerRebuildPipeline::run_with_readiness_for_package_with_features(
				&config.bin_name,
				rebuild_context.package,
				rebuild_context.features,
				rebuild_context.all_features,
				current_child,
				respawn,
				&config.address,
			);
		let (wasm_ok, (server_outcome, new_child)) = tokio::join!(wasm_fut, server_fut);
		let server_ok = server_rebuild_succeeded(&server_outcome);
		if let Some(child) = new_child {
			*current_child = child;
		}
		let server_ready = if server_ok {
			wait_for_server_ready(config.server_address.as_deref()).await
		} else {
			false
		};
		#[cfg(feature = "pages")]
		let browser_ready = wasm_ok
			&& server_ready
			&& (css_only
				|| commit_pending_component_styles(config.component_styles.as_ref(), wasm_ok));
		#[cfg(feature = "pages")]
		let template_fallback_handled = finalize_template_fallback_build(
			template_coordinator.as_deref_mut(),
			template_fallback_generation,
			config,
			browser_ready,
		);
		#[cfg(feature = "pages")]
		if browser_ready && !template_fallback_handled {
			notify_browser_reload(
				config.hmr_tx.as_ref(),
				"Rust rebuild completed successfully",
			);
			refresh_template_baseline_after_full_reload(
				template_coordinator.as_deref_mut(),
				config,
			);
		}
		#[cfg(not(feature = "pages"))]
		let _ = (wasm_ok, server_ready);
	} else if targets.wasm {
		let wasm_ok = wasm_fut.await;
		#[cfg(feature = "pages")]
		let component_styles_committed =
			wasm_ok && commit_pending_component_styles(config.component_styles.as_ref(), wasm_ok);
		#[cfg(feature = "pages")]
		let browser_ready = should_notify_wasm_reload(wasm_ok, css_only, component_styles_committed);
		#[cfg(feature = "pages")]
		let template_fallback_handled = finalize_template_fallback_build(
			template_coordinator.as_deref_mut(),
			template_fallback_generation,
			config,
			browser_ready,
		);
		#[cfg(feature = "pages")]
		if browser_ready && !template_fallback_handled {
			notify_browser_reload(
				config.hmr_tx.as_ref(),
				"WASM rebuild completed successfully",
			);
			refresh_template_baseline_after_full_reload(template_coordinator, config);
		}
		#[cfg(not(feature = "pages"))]
		let _ = wasm_ok;
	} else {
		let (server_outcome, new_child) =
			crate::server_rebuild_pipeline::ServerRebuildPipeline::run_with_readiness_for_package_with_features(
				&config.bin_name,
				rebuild_context.package,
				rebuild_context.features,
				rebuild_context.all_features,
				current_child,
				respawn,
				&config.address,
			)
			.await;
		if let Some(child) = new_child {
			*current_child = child;
		}
		#[cfg(feature = "pages")]
		notify_server_reload_if_ready(&server_outcome, config).await;
		// A server-only rebuild cannot activate pending component-style APIs:
		// the browser still runs the previous WASM bundle. Keep the stylesheet
		// staged until a successful WASM rebuild can commit it atomically.
	}
	// Pipeline failures are recorded as log lines and never propagate as Err;
	// the caller's loop continues unconditionally.
}

#[cfg(feature = "pages")]
fn finalize_template_fallback_build(
	coordinator: Option<&mut TemplateHotReloadCoordinator>,
	generation: Option<reinhardt_pages::hmr::PatchGeneration>,
	config: &WatcherConfig,
	browser_ready: bool,
) -> bool {
	let (Some(coordinator), Some(generation)) = (coordinator, generation) else {
		return false;
	};
	if !browser_ready {
		coordinator.publish_build_failure(
			generation,
			reinhardt_pages::hmr::DiagnosticTarget::WasmRustc,
			"The fallback build did not become ready; the last working page remains active.",
		);
		return true;
	}
	match collect_client_baseline(&config.project_root, &config.roots.src_dirs) {
		Ok(baseline) => {
			let artifact = TemplateBuildArtifact {
				build_id: baseline.build_id,
				manifest_digest: baseline.manifest_digest,
				baseline,
			};
			if let Err(error) = coordinator.install_successful_build(artifact) {
				coordinator.publish_build_failure(
					generation,
					reinhardt_pages::hmr::DiagnosticTarget::Template,
					format!("failed to install the rebuilt template manifest: {error}"),
				);
			}
		}
		Err(error) => coordinator.publish_build_failure(
			generation,
			reinhardt_pages::hmr::DiagnosticTarget::Template,
			format!("failed to collect the rebuilt template manifest: {error}"),
		),
	}
	true
}

#[cfg(feature = "pages")]
fn refresh_template_baseline_after_full_reload(
	coordinator: Option<&mut TemplateHotReloadCoordinator>,
	config: &WatcherConfig,
) {
	let Some(coordinator) = coordinator else {
		return;
	};
	match collect_client_baseline(&config.project_root, &config.roots.src_dirs) {
		Ok(baseline) => coordinator.refresh_successful_baseline(baseline),
		Err(error) => eprintln!(
			"[hot-reload] unable to refresh template patch baseline after full reload: {error}"
		),
	}
}

#[cfg(feature = "pages")]
fn should_notify_wasm_reload(
	wasm_ok: bool,
	css_only: bool,
	component_styles_committed: bool,
) -> bool {
	wasm_ok && (css_only || component_styles_committed)
}

#[cfg(feature = "pages")]
fn can_short_circuit_css_only(
	style_stage: crate::ComponentStyleStageResult,
	metadata_changed: bool,
	component_style_paths_only: bool,
) -> bool {
	style_stage == crate::ComponentStyleStageResult::CssOnly
		&& !metadata_changed
		&& component_style_paths_only
}

#[cfg(feature = "pages")]
fn notify_component_stylesheet_update(hmr_tx: Option<&broadcast::Sender<String>>) {
	if let Some(tx) = hmr_tx {
		let message = reinhardt_pages::hmr::HmrMessage::CssUpdate {
			path: crate::COMPONENT_STYLES_PATH.to_string(),
		};
		if let Ok(json) = message.to_json() {
			let _ = tx.send(json);
		}
	}
}

/// Run the hot-reload watcher loop until shutdown.
///
/// The loop handles three concerns:
///
/// 1. Subscribes the recommended `notify` watcher to every existing
///    `roots.src_dirs` (recursively), `roots.manifest_files`
///    (non-recursively), and `roots.lockfile` when present
///    (non-recursively, so `cargo update` triggers a rebuild even when
///    no path-dep source files change; see issue #4214). Non-existent
///    paths are skipped without error.
/// 2. Awaits debounced events, dispatching each to the WASM pipeline
///    (when `pages_enabled && !no_wasm_rebuild`) and then the server
///    pipeline. Pipeline failures are logged but never returned as `Err`.
/// 3. Honours the `shutdown_rx` oneshot: on shutdown the running child
///    is killed and reaped before returning.
///
/// Only watcher infrastructure errors (e.g. `notify::Watcher::new` failure
/// or a failed `watch` subscription on an existing path) propagate as
/// `Err`.
pub async fn run_watcher(
	ctx: &CommandContext,
	config: &WatcherConfig,
	shutdown_rx: oneshot::Receiver<()>,
	current_child: tokio::process::Child,
	respawn: impl Fn() -> std::io::Result<tokio::process::Child> + Send + Sync,
) -> Result<(), notify::Error> {
	run_watcher_for_package(
		ctx,
		config,
		ServerRebuildContext::default(),
		shutdown_rx,
		current_child,
		respawn,
	)
	.await
}

/// Run the watcher while forwarding the selected Cargo build context to native rebuilds.
pub(crate) async fn run_watcher_for_package(
	ctx: &CommandContext,
	config: &WatcherConfig,
	rebuild_context: ServerRebuildContext<'_>,
	shutdown_rx: oneshot::Receiver<()>,
	mut current_child: tokio::process::Child,
	respawn: impl Fn() -> std::io::Result<tokio::process::Child> + Send + Sync,
) -> Result<(), notify::Error> {
	use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

	// Bounded channel: notify is bursty and we never want a single backed-up
	// watcher to wedge the runtime. 256 is an order of magnitude above
	// observed IDE save bursts.
	let (tx, mut rx) = tokio::sync::mpsc::channel::<Event>(256);

	let mut watcher = RecommendedWatcher::new(
		move |res: Result<Event, notify::Error>| {
			if let Ok(event) = res {
				// `blocking_send` is fine here: notify dispatches callbacks
				// from a dedicated OS thread, not the tokio runtime.
				let _ = tx.blocking_send(event);
			}
		},
		Config::default(),
	)?;

	// Subscribe each root. Missing paths are skipped (workspace members may
	// be referenced in `Cargo.toml` but not yet exist on disk).
	for dir in &config.roots.src_dirs {
		if dir.exists() {
			watcher.watch(dir, RecursiveMode::Recursive)?;
		}
	}
	for manifest in &config.roots.manifest_files {
		if manifest.exists() {
			watcher.watch(manifest, RecursiveMode::NonRecursive)?;
		}
	}
	// Subscribe the workspace Cargo.lock so `cargo update` triggers a
	// rebuild even when no path-dep source files change. See issue #4214.
	if let Some(lockfile) = &config.roots.lockfile
		&& lockfile.exists()
	{
		watcher.watch(lockfile, RecursiveMode::NonRecursive)?;
	}

	let mut shutdown_rx = shutdown_rx;
	#[cfg(feature = "pages")]
	let mut template_coordinator = if config.pages_enabled && !config.no_wasm_rebuild {
		match (
			config.hmr_server.clone(),
			collect_client_baseline(&config.project_root, &config.roots.src_dirs),
		) {
			(Some(server), Ok(baseline)) => {
				let mut coordinator =
					TemplateHotReloadCoordinator::new(config.project_root.clone(), Some(server));
				coordinator.install_initial_baseline(baseline);
				Some(coordinator)
			}
			(Some(_), Err(error)) => {
				ctx.warning(&format!(
					"[hot-reload] template patching is unavailable until the next successful WASM build: {error}"
				));
				None
			}
			(None, _) => None,
		}
	} else {
		None
	};
	#[cfg(feature = "pages")]
	let mut hmr_client_events = config
		.hmr_server
		.as_ref()
		.map(reinhardt_pages::hmr::HmrServer::client_events);
	#[cfg(not(feature = "pages"))]
	let mut hmr_client_events = ();

	loop {
		tokio::select! {
			biased;
			_ = &mut shutdown_rx => {
				let _ = current_child.kill().await;
				let _ = current_child.wait().await;
				return Ok(());
			}
			client_event = next_hmr_client_event(&mut hmr_client_events) => {
				#[cfg(feature = "pages")]
				{
					let fallback = reinhardt_pages::hmr::HmrMessage::from_json(&client_event)
						.ok()
						.and_then(|message| match message {
							reinhardt_pages::hmr::HmrMessage::PatchApplied { generation } => {
								template_coordinator
									.as_mut()
									.map(|coordinator| coordinator.handle_patch_applied(generation));
								None
							}
							reinhardt_pages::hmr::HmrMessage::PatchRejected { generation, reason } => {
								template_coordinator.as_mut().and_then(|coordinator| {
									let outcome = coordinator.handle_patch_rejection(generation, reason);
									matches!(outcome, DispatchOutcome::RebuildStarted(_))
										.then(|| coordinator.pending_rebuild_paths().map(|paths| (generation, paths.to_vec())))
										.flatten()
								})
							}
							_ => None,
						});
					if let Some((generation, paths)) = fallback {
						run_rebuild_for_paths_for_package(
							WatcherRebuildContext {
								command_context: ctx,
								config,
								server_rebuild_context: rebuild_context,
							},
							paths,
							&mut current_child,
							&respawn,
							template_coordinator.as_mut(),
							Some(generation),
						)
						.await;
					}
				}
				#[cfg(not(feature = "pages"))]
				let _ = client_event;
			}
				debounced = debounce_next(&mut rx, config.debounce_window) => {
					let Some(paths) = debounced else {
						// Channel closed: the watcher dropped or the OS torn
						// the subscription down. Treat as graceful shutdown.
						let _ = current_child.kill().await;
						let _ = current_child.wait().await;
						return Ok(());
					};
					#[cfg(feature = "pages")]
					let (patch_sent, fallback_generation) = if can_attempt_template_patch(&paths, config) {
						match template_coordinator.as_mut().map(|coordinator| coordinator.classify_and_dispatch(paths.clone())) {
							Some(DispatchOutcome::PatchSent(_) | DispatchOutcome::IgnoredStale(_)) => (true, None),
							Some(DispatchOutcome::RebuildStarted(generation)) => (false, Some(generation)),
							Some(DispatchOutcome::DiagnosticPublished(generation)) => (false, Some(generation)),
							None => (false, None),
						}
					} else {
						(false, None)
					};
					#[cfg(not(feature = "pages"))]
					let patch_sent = false;
					if !patch_sent {
						run_rebuild_for_paths_for_package(
							WatcherRebuildContext {
								command_context: ctx,
								config,
								server_rebuild_context: rebuild_context,
							},
							paths,
							&mut current_child,
							&respawn,
							#[cfg(feature = "pages")]
							template_coordinator.as_mut(),
							#[cfg(feature = "pages")]
							fallback_generation,
						)
						.await;
					}
				}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use notify::event::{CreateKind, ModifyKind, RemoveKind};
	use notify::{Event, EventKind};
	use rstest::rstest;
	use std::path::PathBuf;
	use tokio::sync::mpsc;

	#[test]
	fn native_server_rebuild_ignores_pages_package_selection() {
		// Arrange and Act
		let context = ServerRebuildContext::for_native_server();

		// Assert
		assert_eq!(context.package, None);
		assert!(context.features.is_empty());
		assert!(!context.all_features);
	}

	fn ev(kind: EventKind, path: &str) -> Event {
		Event {
			kind,
			paths: vec![PathBuf::from(path)],
			attrs: Default::default(),
		}
	}

	#[rstest]
	#[case::rust_file_modify(EventKind::Modify(ModifyKind::Any), "/project/src/main.rs", true)]
	#[case::toml_file_modify(EventKind::Modify(ModifyKind::Any), "/project/Cargo.toml", true)]
	#[case::rust_file_create(EventKind::Create(CreateKind::File), "/project/src/new.rs", true)]
	#[case::rust_file_remove(EventKind::Remove(RemoveKind::File), "/project/src/old.rs", true)]
	#[case::target_dir_rejected(
		EventKind::Modify(ModifyKind::Any),
		"/project/target/debug/main.rs",
		false
	)]
	#[case::git_dir_rejected(
		EventKind::Modify(ModifyKind::Any),
		"/project/.git/objects/abc",
		false
	)]
	#[case::swap_file_rejected(
		EventKind::Modify(ModifyKind::Any),
		"/project/src/main.rs.swp",
		false
	)]
	#[case::markdown_rejected(EventKind::Modify(ModifyKind::Any), "/project/README.md", false)]
	#[case::cargo_lock_modify(EventKind::Modify(ModifyKind::Any), "/project/Cargo.lock", true)]
	#[case::cargo_lock_bak_rejected(
		EventKind::Modify(ModifyKind::Any),
		"/project/Cargo.lock.bak",
		false
	)]
	#[case::generic_lock_rejected(EventKind::Modify(ModifyKind::Any), "/project/foo.lock", false)]
	fn is_relevant_change_filter_cases(
		#[case] kind: EventKind,
		#[case] path: &str,
		#[case] expected: bool,
	) {
		// Arrange
		let event = ev(kind, path);

		// Act
		let actual = is_relevant_change(&event);

		// Assert
		assert_eq!(
			actual, expected,
			"is_relevant_change({path:?}) = {actual}, want {expected}"
		);
	}

	#[tokio::test(flavor = "current_thread", start_paused = true)]
	async fn debounce_coalesces_burst_into_single_trigger() {
		// Arrange: three rapid events, two of them on the same path so the
		// dedup logic also gets exercised.
		let (tx, mut rx) = mpsc::channel::<Event>(8);
		tx.send(ev(EventKind::Modify(ModifyKind::Any), "/p/src/a.rs"))
			.await
			.unwrap();
		tx.send(ev(EventKind::Modify(ModifyKind::Any), "/p/src/a.rs"))
			.await
			.unwrap();
		tx.send(ev(EventKind::Modify(ModifyKind::Any), "/p/src/b.rs"))
			.await
			.unwrap();

		// Act
		let result = debounce_next(&mut rx, Duration::from_millis(300)).await;

		// Assert: returns exactly one Vec containing both unique paths,
		// sorted (BTreeSet ordering).
		assert_eq!(
			result,
			Some(vec![
				PathBuf::from("/p/src/a.rs"),
				PathBuf::from("/p/src/b.rs"),
			]),
		);
	}

	#[tokio::test(flavor = "current_thread", start_paused = true)]
	async fn debounce_waits_for_configured_window() {
		// Arrange
		let (tx, mut rx) = mpsc::channel::<Event>(8);
		tx.send(ev(EventKind::Modify(ModifyKind::Any), "/p/src/a.rs"))
			.await
			.unwrap();
		let started_at = Instant::now();
		let window = Duration::from_millis(120);
		let mut pending = Box::pin(debounce_next(&mut rx, window));

		// Act: the debounce future must not complete before the configured
		// window expires. Paused time keeps this deterministic without a real sleep.
		tokio::select! {
			result = &mut pending => panic!("debounce completed too early: {result:?}"),
			_ = tokio::task::yield_now() => {}
		}
		tokio::time::advance(window).await;

		let result = pending.await;

		// Assert
		assert_eq!(Instant::now() - started_at, window);
		assert_eq!(result, Some(vec![PathBuf::from("/p/src/a.rs")]));
	}

	#[tokio::test(flavor = "current_thread", start_paused = true)]
	async fn debounce_returns_none_when_channel_closed_without_events() {
		// Arrange: drop the sender immediately so the channel closes with no
		// pending messages.
		let (tx, mut rx) = mpsc::channel::<Event>(1);
		drop(tx);

		// Act
		let result = debounce_next(&mut rx, Duration::from_millis(300)).await;

		// Assert
		assert!(result.is_none());
	}

	#[tokio::test(flavor = "current_thread", start_paused = true)]
	async fn debounce_returns_sorted_unique_paths_when_channel_closes_after_first_event() {
		let (tx, mut rx) = mpsc::channel::<Event>(4);
		tx.send(ev(EventKind::Modify(ModifyKind::Any), "/p/src/z.rs"))
			.await
			.expect("first event is queued");
		tx.send(ev(EventKind::Modify(ModifyKind::Any), "/p/src/a.rs"))
			.await
			.expect("second event is queued");
		tx.send(ev(EventKind::Modify(ModifyKind::Any), "/p/src/z.rs"))
			.await
			.expect("duplicate event is queued");
		drop(tx);

		let paths = debounce_next(&mut rx, DEBOUNCE_WINDOW).await;

		assert_eq!(
			paths,
			Some(vec![
				PathBuf::from("/p/src/a.rs"),
				PathBuf::from("/p/src/z.rs"),
			]),
		);
	}

	#[test]
	fn metadata_change_is_detected_when_batched_with_a_stylesheet_source_change() {
		// Arrange
		let paths = vec![
			PathBuf::from("/project/src/styles.rs"),
			PathBuf::from("/project/Cargo.toml"),
		];

		// Act
		let changed = paths_change_cargo_metadata(&paths);

		// Assert
		assert!(
			changed,
			"Cargo metadata must keep the rebuild pipeline active"
		);
	}

	#[cfg(feature = "pages")]
	fn pages_config(no_wasm_rebuild: bool) -> WatcherConfig {
		WatcherConfig {
			project_root: PathBuf::from("/project"),
			bin_name: "manage".to_string(),
			address: "127.0.0.1:8000".to_string(),
			roots: SourceRoots {
				src_dirs: vec![PathBuf::from("/project/src")],
				manifest_files: vec![PathBuf::from("/project/Cargo.toml")],
				lockfile: Some(PathBuf::from("/project/Cargo.lock")),
			},
			debounce_window: DEBOUNCE_WINDOW,
			server_address: None,
			no_wasm_rebuild,
			pages_enabled: true,
			hmr_tx: None,
			hmr_server: None,
			component_styles: None,
		}
	}

	#[cfg(feature = "pages")]
	#[rstest]
	#[case::root_client_rs("/project/src/client.rs", false, true)]
	#[case::root_client_dir("/project/src/client/pages.rs", false, true)]
	#[case::app_client_dir("/project/src/apps/polls/client/page.rs", false, true)]
	#[case::bin_manage("/project/src/bin/manage.rs", true, false)]
	#[case::settings_config("/project/src/config/settings.rs", true, false)]
	#[case::shared_types("/project/src/shared/types.rs", true, true)]
	#[case::server_fn_boundary("/project/src/apps/polls/server_fn.rs", true, true)]
	#[case::manifest("/project/Cargo.toml", true, true)]
	#[case::lockfile("/project/Cargo.lock", true, true)]
	fn rebuild_targets_classify_pages_paths(
		#[case] path: &str,
		#[case] expected_server: bool,
		#[case] expected_wasm: bool,
	) {
		// Arrange
		let config = pages_config(false);

		// Act
		let actual = rebuild_targets_for_paths(&[PathBuf::from(path)], &config);

		// Assert
		assert_eq!(
			actual,
			RebuildTargets {
				server: expected_server,
				wasm: expected_wasm,
			},
			"unexpected rebuild targets for {path}"
		);
	}

	#[cfg(feature = "pages")]
	#[test]
	fn rebuild_targets_keep_client_only_noop_when_wasm_rebuild_disabled() {
		// Arrange
		let config = pages_config(true);

		// Act
		let actual =
			rebuild_targets_for_paths(&[PathBuf::from("/project/src/client/page.rs")], &config);

		// Assert
		assert_eq!(
			actual,
			RebuildTargets {
				server: false,
				wasm: false,
			},
		);
	}

	#[cfg(feature = "pages")]
	#[test]
	fn rebuild_targets_keep_shared_server_rebuild_when_wasm_rebuild_disabled() {
		// Arrange
		let config = pages_config(true);

		// Act
		let actual = rebuild_targets_for_paths(&[PathBuf::from("/project/src/lib.rs")], &config);

		// Assert
		assert_eq!(
			actual,
			RebuildTargets {
				server: true,
				wasm: false,
			},
		);
	}

	#[cfg(feature = "pages")]
	#[test]
	fn css_only_shortcut_requires_a_component_style_only_batch() {
		// Act & Assert
		assert!(can_short_circuit_css_only(
			crate::ComponentStyleStageResult::CssOnly,
			false,
			true,
		));
		assert!(!can_short_circuit_css_only(
			crate::ComponentStyleStageResult::CssOnly,
			false,
			false,
		));
	}

	#[cfg(feature = "pages")]
	#[test]
	fn component_stylesheet_update_sends_css_hmr_message() {
		// Arrange
		let (tx, mut rx) = broadcast::channel::<String>(8);

		// Act
		notify_component_stylesheet_update(Some(&tx));

		// Assert
		let json = rx
			.try_recv()
			.expect("stylesheet update should be broadcast");
		let message: reinhardt_pages::hmr::HmrMessage =
			serde_json::from_str(&json).expect("message should be valid HMR JSON");
		assert_eq!(
			message,
			reinhardt_pages::hmr::HmrMessage::CssUpdate {
				path: crate::COMPONENT_STYLES_PATH.to_string(),
			}
		);
	}

	#[cfg(feature = "pages")]
	#[test]
	fn notify_browser_reload_sends_full_reload_message() {
		// Arrange
		let (tx, mut rx) = broadcast::channel::<String>(8);

		// Act
		let sent = notify_browser_reload(Some(&tx), "WASM rebuild completed successfully");

		// Assert
		let json = rx.try_recv().expect("reload message should be broadcast");
		let message: reinhardt_pages::hmr::HmrMessage =
			serde_json::from_str(&json).expect("message should be valid HMR JSON");
		assert_eq!(
			message,
			reinhardt_pages::hmr::HmrMessage::FullReload {
				reason: "WASM rebuild completed successfully".to_string()
			}
		);
		assert!(sent);
	}

	#[cfg(feature = "pages")]
	#[test]
	fn notify_browser_reload_without_channel_is_noop() {
		// Act & Assert
		assert!(!notify_browser_reload(
			None,
			"Server rebuild completed successfully"
		));
	}

	#[cfg(feature = "pages")]
	#[test]
	fn wasm_rebuild_with_css_only_batch_requires_a_full_reload() {
		assert!(should_notify_wasm_reload(true, true, false));
		assert!(!should_notify_wasm_reload(false, true, false));
	}

	#[cfg(feature = "pages")]
	#[tokio::test]
	async fn server_rebuild_reload_notifies_after_readiness() {
		// Arrange
		let (tx, mut rx) = broadcast::channel::<String>(8);
		let mut config = pages_config(false);
		config.hmr_tx = Some(tx);
		let outcome = crate::server_rebuild_pipeline::ServerRebuildOutcome::Ok {
			duration: Duration::ZERO,
		};

		// Act
		notify_server_reload_if_ready(&outcome, &config).await;

		// Assert
		let json = rx
			.try_recv()
			.expect("server rebuild should notify the browser");
		let message: reinhardt_pages::hmr::HmrMessage =
			serde_json::from_str(&json).expect("message should be valid HMR JSON");
		assert_eq!(
			message,
			reinhardt_pages::hmr::HmrMessage::FullReload {
				reason: "Server rebuild completed successfully".to_string()
			}
		);
	}

	#[cfg(feature = "pages")]
	#[test]
	fn pending_component_styles_are_not_committed_without_a_wasm_rebuild() {
		// Arrange
		let directory = tempfile::tempdir().expect("create temporary package");
		std::fs::create_dir(directory.path().join("src")).expect("create source root");
		std::fs::write(
			directory.path().join("Cargo.toml"),
			"[package]\nname = \"watcher-style-api\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
		)
		.expect("write manifest");
		let source_path = directory.path().join("src/lib.rs");
		std::fs::write(
			&source_path,
			"#[style_def] static STYLES: Styles = style! { .card { color: red; } };\n",
		)
		.expect("write initial source");
		let state = std::sync::Arc::new(std::sync::Mutex::new(
			crate::ComponentStyleState::initialize(directory.path().join("Cargo.toml"), None)
				.expect("initialize component styles"),
		));
		let stylesheet = state
			.lock()
			.expect("lock component styles")
			.generated_root()
			.join(crate::COMPONENT_STYLES_PATH);
		let before = std::fs::read(&stylesheet).expect("read initial stylesheet");
		std::fs::write(
			&source_path,
			"#[style_def] static STYLES: Styles = style! { .card { color: red; } .label { color: blue; } };\n",
		)
		.expect("write API-changing source");
		assert_eq!(
			state.lock().expect("lock component styles").refresh(false),
			crate::ComponentStyleStageResult::RustOrApiChanged
		);

		// Act
		let committed = commit_pending_component_styles(Some(&state), false);

		// Assert
		assert!(!committed);
		assert_eq!(
			std::fs::read(&stylesheet).expect("read retained stylesheet"),
			before
		);
	}
}
