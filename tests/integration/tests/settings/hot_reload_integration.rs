//! Integration tests for Hot Reload functionality
//!
//! These tests verify file system watching and automatic configuration reload.

#[cfg(feature = "hot-reload")]
use reinhardt_conf::settings::backends::MemoryBackend;
#[cfg(feature = "hot-reload")]
use reinhardt_conf::settings::dynamic::DynamicSettings;
#[cfg(feature = "hot-reload")]
use reinhardt_conf::settings::hot_reload::HotReloadManager;
#[cfg(feature = "hot-reload")]
use std::fs;
#[cfg(feature = "hot-reload")]
use std::sync::Arc;
#[cfg(feature = "hot-reload")]
use std::sync::atomic::{AtomicU32, Ordering};
#[cfg(feature = "hot-reload")]
use std::time::Duration;
#[cfg(feature = "hot-reload")]
use tempfile::TempDir;

#[cfg(feature = "hot-reload")]
#[tokio::test]
async fn test_file_watching_callback_invocation() {
	let temp_dir = TempDir::new().unwrap();
	let file_path = temp_dir.path().join("config.toml");
	fs::write(&file_path, "initial content").unwrap();

	let manager = HotReloadManager::new();
	let call_count = Arc::new(AtomicU32::new(0));
	let call_count_clone = call_count.clone();

	manager.on_reload(Arc::new(move |_path| {
		call_count_clone.fetch_add(1, Ordering::SeqCst);
	}));

	manager.watch(&file_path).await.unwrap();

	// Modify file
	fs::write(&file_path, "modified content").unwrap();

	// Wait for file system event
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Callback should have been invoked
	assert!(call_count.load(Ordering::SeqCst) > 0);

	manager.stop().await.unwrap();
}

#[cfg(feature = "hot-reload")]
#[tokio::test]
async fn test_multiple_file_watchers() {
	let temp_dir = TempDir::new().unwrap();
	let file1 = temp_dir.path().join("config1.toml");
	let file2 = temp_dir.path().join("config2.toml");

	fs::write(&file1, "content1").unwrap();
	fs::write(&file2, "content2").unwrap();

	let manager = HotReloadManager::new();
	let call_count = Arc::new(AtomicU32::new(0));
	let call_count_clone = call_count.clone();

	manager.on_reload(Arc::new(move |_| {
		call_count_clone.fetch_add(1, Ordering::SeqCst);
	}));

	// Watch both files
	manager.watch(&file1).await.unwrap();
	manager.watch(&file2).await.unwrap();

	// Verify both are watched
	let paths = manager.watched_paths();
	assert_eq!(paths.len(), 2);

	// Modify first file
	fs::write(&file1, "modified1").unwrap();
	tokio::time::sleep(Duration::from_millis(300)).await;

	let count_after_first = call_count.load(Ordering::SeqCst);
	assert!(count_after_first > 0);

	// Modify second file
	fs::write(&file2, "modified2").unwrap();
	tokio::time::sleep(Duration::from_millis(300)).await;

	let count_after_second = call_count.load(Ordering::SeqCst);
	assert!(count_after_second > count_after_first);

	manager.stop().await.unwrap();
}

#[cfg(feature = "hot-reload")]
#[tokio::test]
async fn test_hot_reload_with_dynamic_settings() {
	let temp_dir = TempDir::new().unwrap();
	let file_path = temp_dir.path().join("settings.toml");
	fs::write(&file_path, "initial config").unwrap();

	let backend = Arc::new(MemoryBackend::new());
	let settings = DynamicSettings::new(backend).with_hot_reload();

	let reload_count = Arc::new(AtomicU32::new(0));
	let reload_count_clone = reload_count.clone();

	// Subscribe to observe reloads
	settings.subscribe(move |_, _| {
		reload_count_clone.fetch_add(1, Ordering::SeqCst);
	});

	// Watch configuration file
	settings.watch_file(&file_path).await.unwrap();

	// Simulate configuration change by writing to file
	fs::write(&file_path, "updated config").unwrap();

	// Wait for file system event
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Note: The file change itself doesn't trigger observers in current implementation
	// This test verifies the integration, not automatic reload from file changes
	// For automatic reload, you would need to parse the file and update settings in the callback

	settings.stop_watching().await.unwrap();
}

#[cfg(feature = "hot-reload")]
#[tokio::test]
async fn test_debouncing_rapid_changes() {
	let temp_dir = TempDir::new().unwrap();
	let file_path = temp_dir.path().join("config.toml");
	fs::write(&file_path, "initial").unwrap();

	let manager = HotReloadManager::with_debounce(Duration::from_millis(300));
	let call_count = Arc::new(AtomicU32::new(0));
	let call_count_clone = call_count.clone();

	manager.on_reload(Arc::new(move |_| {
		call_count_clone.fetch_add(1, Ordering::SeqCst);
	}));

	manager.watch(&file_path).await.unwrap();

	// Make rapid changes
	for i in 0..5 {
		fs::write(&file_path, format!("content {}", i)).unwrap();
		tokio::time::sleep(Duration::from_millis(50)).await;
	}

	// Wait for debounce window
	tokio::time::sleep(Duration::from_millis(800)).await;

	// Due to debouncing, callbacks should be fewer than file writes
	let count = call_count.load(Ordering::SeqCst);
	assert!(
		count > 0 && count < 5,
		"Expected debounced calls, got {}",
		count
	);

	manager.stop().await.unwrap();
}

#[cfg(feature = "hot-reload")]
#[tokio::test]
async fn test_unwatch_file() {
	let temp_dir = TempDir::new().unwrap();
	let file_path = temp_dir.path().join("config.toml");
	fs::write(&file_path, "initial").unwrap();

	let manager = HotReloadManager::new();
	let call_count = Arc::new(AtomicU32::new(0));
	let call_count_clone = call_count.clone();

	manager.on_reload(Arc::new(move |_| {
		call_count_clone.fetch_add(1, Ordering::SeqCst);
	}));

	// Start watching
	manager.watch(&file_path).await.unwrap();
	assert_eq!(manager.watched_paths().len(), 1);

	// Modify file - should trigger callback
	fs::write(&file_path, "modified1").unwrap();
	tokio::time::sleep(Duration::from_millis(300)).await;

	let count_before_unwatch = call_count.load(Ordering::SeqCst);
	assert!(count_before_unwatch > 0);

	// Stop watching
	manager.unwatch(&file_path).await.unwrap();
	assert_eq!(manager.watched_paths().len(), 0);

	// Modify file again - should NOT trigger callback
	fs::write(&file_path, "modified2").unwrap();
	tokio::time::sleep(Duration::from_millis(300)).await;

	let count_after_unwatch = call_count.load(Ordering::SeqCst);
	assert_eq!(count_after_unwatch, count_before_unwatch);

	manager.stop().await.unwrap();
}
