//! File system watcher for HMR change detection.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher, event::ModifyKind};
use tokio::sync::mpsc;

use super::change_kind::ChangeKind;
use super::config::HmrConfig;

/// A file change event emitted by the watcher.
#[derive(Debug, Clone)]
pub struct FileChangeEvent {
	/// The path of the changed file.
	pub path: PathBuf,
	/// The classified kind of change.
	pub kind: ChangeKind,
}

/// Watches the file system for changes and emits classified change events.
pub struct FileWatcher {
	config: Arc<HmrConfig>,
	// Keep the watcher alive as long as FileWatcher exists
	_watcher: RecommendedWatcher,
	/// Channel receiver for file change events.
	pub rx: mpsc::UnboundedReceiver<FileChangeEvent>,
}

impl FileWatcher {
	/// Creates a new file watcher that monitors paths specified in the config.
	///
	/// Returns the watcher and immediately begins monitoring. File change events
	/// are received through the `rx` field.
	pub fn new(config: HmrConfig) -> Result<Self, notify::Error> {
		let config = Arc::new(config);
		let (tx, rx) = mpsc::unbounded_channel();

		let debounce_duration = Duration::from_millis(config.debounce_ms);
		let notify_config = Config::default().with_poll_interval(debounce_duration);

		let tx_clone = tx.clone();
		let mut watcher = RecommendedWatcher::new(
			move |res: Result<notify::Event, notify::Error>| {
				if let Ok(event) = res {
					// Only process create, modify, and remove events
					let dominated_event = matches!(
						event.kind,
						notify::EventKind::Create(_)
							| notify::EventKind::Modify(ModifyKind::Data(_))
							| notify::EventKind::Remove(_)
					);
					if !dominated_event {
						return;
					}

					for path in event.paths {
						let kind = ChangeKind::from_path(&path);
						let change_event = FileChangeEvent { path, kind };
						// Ignore send errors (receiver may have been dropped)
						let _ = tx_clone.send(change_event);
					}
				}
			},
			notify_config,
		)?;

		// Start watching configured paths
		for watch_path in &config.watch_paths {
			if watch_path.exists() {
				watcher.watch(watch_path, RecursiveMode::Recursive)?;
			}
		}

		Ok(Self {
			config,
			_watcher: watcher,
			rx,
		})
	}

	/// Returns a reference to the HMR configuration.
	pub fn config(&self) -> &HmrConfig {
		&self.config
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::fs;
	use tempfile::TempDir;

	#[rstest]
	fn test_file_watcher_creation() {
		// Arrange
		let tmp_dir = TempDir::new().unwrap();
		let config = HmrConfig::builder()
			.watch_path(tmp_dir.path().to_path_buf())
			.debounce_ms(100)
			.build();

		// Act
		let watcher = FileWatcher::new(config);

		// Assert
		assert!(watcher.is_ok());
	}

	#[rstest]
	fn test_file_watcher_config_access() {
		// Arrange
		let tmp_dir = TempDir::new().unwrap();
		let config = HmrConfig::builder()
			.watch_path(tmp_dir.path().to_path_buf())
			.ws_port(9999)
			.build();

		// Act
		let watcher = FileWatcher::new(config).unwrap();

		// Assert
		assert_eq!(watcher.config().ws_port, 9999);
	}

	#[rstest]
	#[tokio::test]
	async fn test_file_watcher_detects_changes() {
		// Arrange
		let tmp_dir = TempDir::new().unwrap();
		let config = HmrConfig::builder()
			.watch_path(tmp_dir.path().to_path_buf())
			.debounce_ms(50)
			.build();

		let mut watcher = FileWatcher::new(config).unwrap();

		// Act - create a new CSS file
		let css_path = tmp_dir.path().join("test.css");
		fs::write(&css_path, "body { color: red; }").unwrap();

		// Wait for the event (with timeout)
		let event = tokio::time::timeout(Duration::from_secs(5), watcher.rx.recv()).await;

		// Assert
		assert!(
			event.is_ok(),
			"Should receive file change event within timeout"
		);
		if let Ok(Some(change)) = event {
			assert_eq!(change.kind, ChangeKind::Css);
		}
	}

	#[rstest]
	fn test_file_watcher_nonexistent_path_skipped() {
		// Arrange - use a path that doesn't exist
		let config = HmrConfig::builder()
			.watch_path("/tmp/reinhardt_hmr_nonexistent_path_12345")
			.build();

		// Act - should not error because nonexistent paths are skipped
		let watcher = FileWatcher::new(config);

		// Assert
		assert!(watcher.is_ok());
	}
}
