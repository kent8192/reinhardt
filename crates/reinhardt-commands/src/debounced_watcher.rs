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
use std::future::Future;
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

/// Observable result of dispatching one debounced rebuild batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RebuildDispatchOutcome {
	/// No selected pipeline needed work.
	NoTargets,
	/// A static Pages patch was delivered without starting a rebuild pipeline.
	StaticPatchSent,
	/// The selected pipelines completed and may have delivered a full reload.
	Rebuilt {
		/// WASM pipeline result when the target was selected.
		wasm_succeeded: Option<bool>,
		/// Server pipeline result when the target was selected.
		server_succeeded: Option<bool>,
		/// Whether a browser received the full reload message.
		browser_reloaded: bool,
	},
}

async fn dispatch_rebuild_with<StaticPatch, BrowserReload, WasmFuture, ServerFuture>(
	targets: RebuildTargets,
	paths: &[PathBuf],
	mut static_patch: StaticPatch,
	rebuild_wasm: WasmFuture,
	rebuild_server: ServerFuture,
	mut browser_reload: BrowserReload,
) -> RebuildDispatchOutcome
where
	StaticPatch: FnMut(&[PathBuf], RebuildTargets) -> bool,
	BrowserReload: FnMut(&str) -> bool,
	WasmFuture: Future<Output = bool>,
	ServerFuture: Future<Output = bool>,
{
	if !targets.has_work() {
		return RebuildDispatchOutcome::NoTargets;
	}
	if static_patch(paths, targets) {
		return RebuildDispatchOutcome::StaticPatchSent;
	}

	let (wasm_succeeded, server_succeeded, browser_reloaded) = match (targets.wasm, targets.server)
	{
		(true, true) => {
			let (wasm_succeeded, server_succeeded) = tokio::join!(rebuild_wasm, rebuild_server);
			let browser_reloaded = if wasm_succeeded && server_succeeded {
				browser_reload("Rust rebuild completed successfully")
			} else {
				false
			};
			(
				Some(wasm_succeeded),
				Some(server_succeeded),
				browser_reloaded,
			)
		}
		(true, false) => {
			let wasm_succeeded = rebuild_wasm.await;
			let browser_reloaded = if wasm_succeeded {
				browser_reload("WASM rebuild completed successfully")
			} else {
				false
			};
			(Some(wasm_succeeded), None, browser_reloaded)
		}
		(false, true) => {
			let server_succeeded = rebuild_server.await;
			let browser_reloaded = if server_succeeded {
				browser_reload("Server rebuild completed successfully")
			} else {
				false
			};
			(None, Some(server_succeeded), browser_reloaded)
		}
		(false, false) => unreachable!("has_work excludes an empty rebuild target"),
	};

	RebuildDispatchOutcome::Rebuilt {
		wasm_succeeded,
		server_succeeded,
		browser_reloaded,
	}
}

/// Configuration for `run_watcher`.
pub struct WatcherConfig {
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
fn notify_static_page_patch(
	hmr_tx: Option<&broadcast::Sender<String>>,
	paths: &[PathBuf],
	targets: RebuildTargets,
) -> bool {
	if hmr_tx.is_none() || targets.server || !targets.wasm {
		return false;
	}
	let Some(html) = crate::page_hot_patch::render_static_page_patch(paths) else {
		return false;
	};
	let msg = reinhardt_pages::hmr::HmrMessage::HtmlReplace {
		selector: "#app".to_string(),
		html,
	};
	if let Ok(json) = msg.to_json()
		&& let Some(tx) = hmr_tx
	{
		return tx.send(json).is_ok();
	}
	false
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
	ctx.info(&format!(
		"[hot-reload] change detected ({} path(s))",
		paths.len()
	));
	let targets = rebuild_targets_for_paths(&paths, config);
	let outcome = dispatch_rebuild_with(
		targets,
		&paths,
		|paths, targets| {
			#[cfg(feature = "pages")]
			{
				notify_static_page_patch(config.hmr_tx.as_ref(), paths, targets)
			}
			#[cfg(not(feature = "pages"))]
			{
				let _ = (paths, targets);
				false
			}
		},
		async {
			#[cfg(feature = "pages")]
			{
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
				wasm_rebuild_succeeded(&outcome)
			}
			#[cfg(not(feature = "pages"))]
			{
				true
			}
		},
		async {
			let (server_outcome, new_child) =
				crate::server_rebuild_pipeline::ServerRebuildPipeline::run_with_readiness(
					&config.bin_name,
					current_child,
					respawn,
					&config.address,
				)
				.await;
			let server_ok = server_rebuild_succeeded(&server_outcome);
			if let Some(child) = new_child {
				*current_child = child;
			}
			if server_ok {
				wait_for_server_ready(config.server_address.as_deref()).await
			} else {
				false
			}
		},
		|reason| {
			#[cfg(feature = "pages")]
			{
				notify_browser_reload(config.hmr_tx.as_ref(), reason)
			}
			#[cfg(not(feature = "pages"))]
			{
				let _ = reason;
				false
			}
		},
	)
	.await;

	if matches!(outcome, RebuildDispatchOutcome::NoTargets) {
		ctx.info("[hot-reload] no rebuild target matched; waiting for next change");
		return;
	}
	if matches!(outcome, RebuildDispatchOutcome::StaticPatchSent) {
		ctx.info("[hot-reload] static page patch sent without rebuilding WASM");
	}
	// Pipeline failures are recorded as log lines and never propagate as Err;
	// the caller's loop continues unconditionally.
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

	loop {
		tokio::select! {
			biased;
			_ = &mut shutdown_rx => {
				let _ = current_child.kill().await;
				let _ = current_child.wait().await;
				return Ok(());
			}
				debounced = debounce_next(&mut rx, config.debounce_window) => {
					let Some(paths) = debounced else {
						// Channel closed: the watcher dropped or the OS torn
						// the subscription down. Treat as graceful shutdown.
						let _ = current_child.kill().await;
						let _ = current_child.wait().await;
						return Ok(());
					};
					run_rebuild_for_paths(ctx, config, paths, &mut current_child, &respawn).await;
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
		// Arrange
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

		// Act
		let paths = debounce_next(&mut rx, DEBOUNCE_WINDOW).await;

		// Assert
		assert_eq!(
			paths,
			Some(vec![
				PathBuf::from("/p/src/a.rs"),
				PathBuf::from("/p/src/z.rs"),
			]),
		);
	}

	#[cfg(feature = "pages")]
	#[rstest]
	#[case::workspace_manifest("/project/Cargo.toml", RebuildTargets { server: true, wasm: true })]
	#[case::nested_manifest("/project/apps/store/Cargo.toml", RebuildTargets { server: true, wasm: true })]
	#[case::static_asset("/project/static/site.css", RebuildTargets { server: true, wasm: true })]
	#[case::page_route("/project/src/apps/store/routes.rs", RebuildTargets { server: true, wasm: true })]
	#[case::page_component("/project/src/apps/store/client/components/cart.rs", RebuildTargets { server: false, wasm: true })]
	#[case::page_template("/project/src/client/templates/store.rs", RebuildTargets { server: false, wasm: true })]
	#[case::hidden_rust_file("/project/src/.generated.rs", RebuildTargets { server: true, wasm: true })]
	#[case::target_artifact("/project/target/debug/app.rs", RebuildTargets { server: true, wasm: true })]
	#[case::editor_swap("/project/src/lib.rs.swp", RebuildTargets { server: true, wasm: true })]
	#[case::unrelated_extension("/project/README.md", RebuildTargets { server: true, wasm: true })]
	fn rebuild_targets_classification_table_covers_path_kinds(
		#[case] path: &str,
		#[case] expected: RebuildTargets,
	) {
		// Arrange
		let config = pages_config(false);

		// Act
		let actual = rebuild_targets_for_paths(&[PathBuf::from(path)], &config);

		// Assert
		assert_eq!(actual, expected, "unexpected targets for {path}");
	}

	#[cfg(feature = "pages")]
	fn pages_config(no_wasm_rebuild: bool) -> WatcherConfig {
		WatcherConfig {
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

	#[test]
	fn rebuild_targets_without_pages_only_restart_the_server() {
		// Arrange
		let config = WatcherConfig {
			bin_name: "manage".to_string(),
			address: "127.0.0.1:8000".to_string(),
			roots: SourceRoots {
				src_dirs: vec![PathBuf::from("/project/src")],
				manifest_files: vec![PathBuf::from("/project/Cargo.toml")],
				lockfile: Some(PathBuf::from("/project/Cargo.lock")),
			},
			debounce_window: DEBOUNCE_WINDOW,
			server_address: None,
			no_wasm_rebuild: false,
			#[cfg(feature = "pages")]
			pages_enabled: false,
			#[cfg(feature = "pages")]
			hmr_tx: None,
		};

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
	#[tokio::test]
	async fn dispatcher_runs_server_when_wasm_fails_without_reloading_browser() {
		// Arrange
		let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
		let static_calls = calls.clone();
		let wasm_calls = calls.clone();
		let server_calls = calls.clone();
		let reload_calls = calls.clone();

		// Act
		let outcome = dispatch_rebuild_with(
			RebuildTargets {
				server: true,
				wasm: true,
			},
			&[PathBuf::from("/project/src/lib.rs")],
			move |_, _| {
				static_calls
					.lock()
					.expect("call lock is available")
					.push("static");
				false
			},
			{
				let calls = wasm_calls.clone();
				async move {
					calls.lock().expect("call lock is available").push("wasm");
					false
				}
			},
			{
				let calls = server_calls.clone();
				async move {
					calls.lock().expect("call lock is available").push("server");
					true
				}
			},
			move |_| {
				reload_calls
					.lock()
					.expect("call lock is available")
					.push("reload");
				true
			},
		)
		.await;

		// Assert
		assert_eq!(
			outcome,
			RebuildDispatchOutcome::Rebuilt {
				wasm_succeeded: Some(false),
				server_succeeded: Some(true),
				browser_reloaded: false,
			},
		);
		assert_eq!(
			*calls.lock().expect("call lock is available"),
			vec!["static", "wasm", "server"],
		);
	}

	#[cfg(feature = "pages")]
	#[tokio::test]
	async fn dispatcher_keeps_failure_local_and_returns_server_failure_outcome() {
		// Arrange
		let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
		let static_calls = calls.clone();
		let server_calls = calls.clone();

		// Act
		let outcome = dispatch_rebuild_with(
			RebuildTargets {
				server: true,
				wasm: false,
			},
			&[PathBuf::from("/project/src/bin/manage.rs")],
			move |_, _| {
				static_calls
					.lock()
					.expect("call lock is available")
					.push("static");
				false
			},
			async { unreachable!("server-only target must not start WASM") },
			{
				let calls = server_calls.clone();
				async move {
					calls.lock().expect("call lock is available").push("server");
					false
				}
			},
			|_| unreachable!("failed server rebuild must not reload the browser"),
		)
		.await;

		// Assert
		assert_eq!(
			outcome,
			RebuildDispatchOutcome::Rebuilt {
				wasm_succeeded: None,
				server_succeeded: Some(false),
				browser_reloaded: false,
			},
		);
		assert_eq!(
			*calls.lock().expect("call lock is available"),
			vec!["static", "server"],
		);
	}

	#[cfg(feature = "pages")]
	#[tokio::test]
	async fn dispatcher_gives_successful_static_patch_precedence_over_pipelines() {
		// Arrange
		let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
		let static_calls = calls.clone();

		// Act
		let outcome = dispatch_rebuild_with(
			RebuildTargets {
				server: false,
				wasm: true,
			},
			&[PathBuf::from("/project/src/client.rs")],
			move |_, _| {
				static_calls
					.lock()
					.expect("call lock is available")
					.push("static");
				true
			},
			async { unreachable!("successful static patch must skip WASM rebuild") },
			async { unreachable!("successful static patch must skip server rebuild") },
			|_| unreachable!("static patch must not send a full browser reload"),
		)
		.await;

		// Assert
		assert_eq!(outcome, RebuildDispatchOutcome::StaticPatchSent);
		assert_eq!(
			*calls.lock().expect("call lock is available"),
			vec!["static"],
		);
	}

	#[cfg(feature = "pages")]
	#[tokio::test]
	async fn dispatcher_records_no_browser_delivery_when_hmr_sender_is_absent() {
		// Arrange
		let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
		let static_calls = calls.clone();
		let wasm_calls = calls.clone();
		let reload_calls = calls.clone();

		// Act
		let outcome = dispatch_rebuild_with(
			RebuildTargets {
				server: false,
				wasm: true,
			},
			&[PathBuf::from("/project/src/client.rs")],
			move |_, _| {
				static_calls
					.lock()
					.expect("call lock is available")
					.push("static");
				false
			},
			{
				let calls = wasm_calls.clone();
				async move {
					calls.lock().expect("call lock is available").push("wasm");
					true
				}
			},
			async { unreachable!("WASM-only target must not restart the server") },
			move |reason| {
				assert_eq!(reason, "WASM rebuild completed successfully");
				reload_calls
					.lock()
					.expect("call lock is available")
					.push("reload");
				false
			},
		)
		.await;

		// Assert
		assert_eq!(
			outcome,
			RebuildDispatchOutcome::Rebuilt {
				wasm_succeeded: Some(true),
				server_succeeded: None,
				browser_reloaded: false,
			},
		);
		assert_eq!(
			*calls.lock().expect("call lock is available"),
			vec!["static", "wasm", "reload"],
		);
	}

	#[cfg(feature = "pages")]
	#[test]
	fn notify_browser_reload_sends_full_reload_message() {
		// Arrange
		let (tx, mut rx) = broadcast::channel::<String>(8);

		// Act
		notify_browser_reload(Some(&tx), "WASM rebuild completed successfully");

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
	}

	#[cfg(feature = "pages")]
	#[test]
	fn notify_browser_reload_without_channel_is_noop() {
		// Act & Assert
		notify_browser_reload(None, "Server rebuild completed successfully");
	}

	#[cfg(feature = "pages")]
	#[test]
	fn notify_static_page_patch_sends_html_replace_for_wasm_only_static_page() {
		// Arrange
		let temp_dir = tempfile::tempdir().expect("tempdir should be created");
		let client_path = temp_dir.path().join("src").join("client.rs");
		std::fs::create_dir_all(
			client_path
				.parent()
				.expect("client path should have parent"),
		)
		.expect("client dir should be created");
		std::fs::write(
			&client_path,
			r#"
				use reinhardt_pages::page;

				fn home_page() -> Page {
					page!(|| {
						div {
							id: "route-home",
							"Updated"
						}
					})()
				}
			"#,
		)
		.expect("client page fixture should be written");
		let (tx, mut rx) = broadcast::channel::<String>(8);

		// Act
		let sent = notify_static_page_patch(
			Some(&tx),
			&[client_path],
			RebuildTargets {
				server: false,
				wasm: true,
			},
		);

		// Assert
		assert!(sent, "static page patch should be sent");
		let json = rx
			.try_recv()
			.expect("html patch message should be broadcast");
		let message: reinhardt_pages::hmr::HmrMessage =
			serde_json::from_str(&json).expect("message should be valid HMR JSON");
		assert_eq!(
			message,
			reinhardt_pages::hmr::HmrMessage::HtmlReplace {
				selector: "#app".to_string(),
				html: r#"<div id="route-home">Updated</div>"#.to_string(),
			}
		);
	}

	#[cfg(feature = "pages")]
	#[test]
	fn notify_static_page_patch_falls_back_for_server_target() {
		// Arrange
		let (tx, _rx) = broadcast::channel::<String>(8);

		// Act
		let sent = notify_static_page_patch(
			Some(&tx),
			&[PathBuf::from("/project/src/lib.rs")],
			RebuildTargets {
				server: true,
				wasm: true,
			},
		);

		// Assert
		assert!(!sent, "shared files must keep the rebuild path");
	}

	#[cfg(feature = "pages")]
	#[test]
	fn notify_static_page_patch_falls_back_without_hmr_receiver() {
		// Arrange
		let temp_dir = tempfile::tempdir().expect("tempdir should be created");
		let client_path = temp_dir.path().join("src").join("client.rs");
		std::fs::create_dir_all(
			client_path
				.parent()
				.expect("client path should have parent"),
		)
		.expect("client dir should be created");
		std::fs::write(
			&client_path,
			r#"
				use reinhardt_pages::page;

				fn home_page() -> Page {
					page!(|| {
						div { "Updated" }
					})()
				}
			"#,
		)
		.expect("client page fixture should be written");
		let (tx, rx) = broadcast::channel::<String>(8);
		drop(rx);

		// Act
		let sent = notify_static_page_patch(
			Some(&tx),
			&[client_path],
			RebuildTargets {
				server: false,
				wasm: true,
			},
		);

		// Assert
		assert!(
			!sent,
			"hot patch must fall back to WASM rebuild when no browser receives it"
		);
	}
}
