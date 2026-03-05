//! Utility functions for the admin CLI.
//!
//! Provides rollback helpers used by the formatting commands to restore
//! files when an error occurs mid-operation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Restore a set of files to their original contents, logging any errors.
///
/// Only files present in `original_contents` are restored. All failures
/// are collected and returned together so that a single write error does
/// not prevent the remaining files from being rolled back.
///
/// # Returns
///
/// A list of `(path, error)` pairs for any writes that failed.
pub(crate) fn rollback_files(
	modified_files: &[PathBuf],
	original_contents: &HashMap<PathBuf, String>,
) -> Vec<(PathBuf, std::io::Error)> {
	let mut errors = Vec::new();
	for file_path in modified_files {
		if let Some(original) = original_contents.get(file_path)
			&& let Err(e) = std::fs::write(file_path, original)
		{
			eprintln!("Warning: failed to rollback {}: {}", file_path.display(), e);
			errors.push((file_path.clone(), e));
		}
	}
	errors
}

/// Log rollback errors in a user-visible way.
///
/// If `errors` is non-empty, prints a summary of files that could not
/// be rolled back.
pub(crate) fn report_rollback_errors(errors: &[(PathBuf, std::io::Error)]) {
	if errors.is_empty() {
		return;
	}
	eprintln!(
		"Warning: {} file(s) could not be rolled back:",
		errors.len()
	);
	for (path, err) in errors {
		eprintln!("  - {}: {}", path.display(), err);
	}
}

/// Write content to a file atomically by writing to a temporary file first, then renaming.
///
/// This prevents data corruption if the write is interrupted (e.g., by a signal or power
/// failure). The rename operation is atomic on the same filesystem.
///
/// Original file permissions are preserved after the write.
pub(crate) fn atomic_write(path: &Path, content: &str) -> std::io::Result<()> {
	// Preserve original permissions before overwrite (if the file exists)
	let original_perms = std::fs::metadata(path).ok().map(|m| m.permissions());

	// Write to a temporary file in the same directory to ensure same filesystem
	let tmp_path = path.with_extension("tmp");
	std::fs::write(&tmp_path, content)?;

	// Atomically rename the temp file over the target
	if let Err(e) = std::fs::rename(&tmp_path, path) {
		// Clean up the temp file if rename fails
		let _ = std::fs::remove_file(&tmp_path);
		return Err(e);
	}

	// Restore original file permissions
	if let Some(perms) = original_perms {
		std::fs::set_permissions(path, perms)?;
	}

	Ok(())
}

/// RAII guard that cleans up a backup file if the operation is not committed.
///
/// Call `commit()` to indicate the operation succeeded and the backup should be kept.
/// If dropped without committing, the backup file is automatically deleted to prevent
/// orphaned backup files on failure.
pub(crate) struct BackupGuard {
	backup_path: PathBuf,
	committed: bool,
}

impl BackupGuard {
	/// Create a new backup guard. The backup file at `backup_path` must already exist.
	pub(crate) fn new(backup_path: PathBuf) -> Self {
		Self {
			backup_path,
			committed: false,
		}
	}

	/// Mark the backup as committed, preventing automatic cleanup on drop.
	pub(crate) fn commit(&mut self) {
		self.committed = true;
	}
}

impl Drop for BackupGuard {
	fn drop(&mut self) {
		if !self.committed {
			let _ = std::fs::remove_file(&self.backup_path);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_rollback_files_restores_modified() {
		// Arrange
		let dir = tempfile::tempdir().expect("failed to create temp dir");
		let file1 = dir.path().join("file1.rs");
		let file2 = dir.path().join("file2.rs");

		std::fs::write(&file1, "original1").expect("write file1");
		std::fs::write(&file2, "original2").expect("write file2");

		let mut originals = HashMap::new();
		originals.insert(file1.clone(), "original1".to_string());
		originals.insert(file2.clone(), "original2".to_string());

		// Simulate modification
		std::fs::write(&file1, "modified1").expect("modify file1");

		// Act
		let errors = rollback_files(&[file1.clone()], &originals);

		// Assert
		assert!(errors.is_empty(), "rollback should succeed");
		assert_eq!(
			std::fs::read_to_string(&file1).unwrap(),
			"original1",
			"file1 should be restored"
		);
		assert_eq!(
			std::fs::read_to_string(&file2).unwrap(),
			"original2",
			"file2 should remain unchanged since it was not in modified list"
		);
	}

	#[rstest]
	fn test_rollback_files_skips_untracked() {
		// Arrange
		let originals = HashMap::new();
		let nonexistent = PathBuf::from("/tmp/reinhardt-test-nonexistent-file.rs");

		// Act
		let errors = rollback_files(&[nonexistent], &originals);

		// Assert
		assert!(errors.is_empty(), "should skip files not in originals map");
	}
}
