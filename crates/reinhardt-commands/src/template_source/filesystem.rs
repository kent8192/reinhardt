//! Filesystem-backed template source with path-traversal protection.

use super::{TemplateEntry, TemplateSource};
use crate::{CommandError, CommandResult};
use std::borrow::Cow;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
/// Template source backed by a directory on the local filesystem.
pub struct FilesystemSource {
	root: PathBuf,
}

fn reject_traversal(rel: &Path) -> CommandResult<()> {
	for comp in rel.components() {
		match comp {
			Component::Normal(_) | Component::CurDir => {}
			_ => {
				return Err(CommandError::ExecutionError(format!(
					"template path escapes root: {}",
					rel.display()
				)));
			}
		}
	}
	Ok(())
}

impl FilesystemSource {
	/// Create a new filesystem source rooted at `root`. Errors if `root` does
	/// not exist or is not a directory.
	pub fn new(root: impl Into<PathBuf>) -> CommandResult<Self> {
		let root = root.into();
		let canonical = fs::canonicalize(&root).map_err(|e| {
			CommandError::ExecutionError(format!(
				"template root does not exist or is not readable: {} ({})",
				root.display(),
				e
			))
		})?;
		if !canonical.is_dir() {
			return Err(CommandError::ExecutionError(format!(
				"template root is not a directory: {}",
				canonical.display()
			)));
		}
		Ok(Self { root: canonical })
	}

	fn resolve(&self, rel: &Path) -> CommandResult<PathBuf> {
		reject_traversal(rel)?;
		let path = self.root.join(rel);
		if path == self.root {
			return Ok(path);
		}
		let canonical = fs::canonicalize(&path).map_err(|e| {
			CommandError::ExecutionError(format!(
				"template path does not exist or is not readable: {} ({})",
				path.display(),
				e
			))
		})?;
		if !canonical.starts_with(&self.root) {
			return Err(CommandError::ExecutionError(format!(
				"template path escapes root: {}",
				rel.display()
			)));
		}
		Ok(canonical)
	}
}

impl TemplateSource for FilesystemSource {
	fn list_entries(&self, rel: &Path) -> CommandResult<Vec<TemplateEntry>> {
		let dir = self.resolve(rel)?;
		if !dir.is_dir() {
			return Err(CommandError::ExecutionError(format!(
				"not a directory: {}",
				dir.display()
			)));
		}
		let read = fs::read_dir(&dir).map_err(|e| {
			CommandError::ExecutionError(format!("read_dir {}: {}", dir.display(), e))
		})?;
		let mut out = Vec::new();
		for entry in read {
			let entry =
				entry.map_err(|e| CommandError::ExecutionError(format!("dir entry: {}", e)))?;
			let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
			let name = entry.file_name();
			let rel_path = rel.join(name);
			out.push(TemplateEntry { rel_path, is_dir });
		}
		Ok(out)
	}

	fn read_file(&self, rel: &Path) -> CommandResult<Cow<'_, [u8]>> {
		let path = self.resolve(rel)?;
		let bytes = fs::read(&path)
			.map_err(|e| CommandError::ExecutionError(format!("read {}: {}", path.display(), e)))?;
		Ok(Cow::Owned(bytes))
	}

	fn exists(&self, rel: &Path) -> bool {
		self.resolve(rel).map(|p| p.exists()).unwrap_or(false)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;
	use std::fs;
	use tempfile::TempDir;

	struct Harness {
		_tmp: TempDir,
		source: FilesystemSource,
	}

	#[fixture]
	fn harness() -> Harness {
		let tmp = TempDir::new().expect("tempdir");
		let root = tmp.path();
		fs::create_dir_all(root.join("sub")).unwrap();
		fs::write(root.join("a.txt"), b"hello").unwrap();
		fs::write(root.join("sub/b.txt"), b"world").unwrap();
		let source = FilesystemSource::new(root).expect("fs source");
		Harness { _tmp: tmp, source }
	}

	#[rstest]
	fn rejects_missing_root() {
		// Act
		let res = FilesystemSource::new("/definitely/does/not/exist_xyz");

		// Assert
		assert!(res.is_err());
	}

	#[rstest]
	fn lists_root_and_subdir(harness: Harness) {
		// Act
		let root = harness.source.list_entries(Path::new("")).unwrap();
		let sub = harness.source.list_entries(Path::new("sub")).unwrap();

		// Assert
		assert!(
			root.iter()
				.any(|e| e.rel_path == PathBuf::from("a.txt") && !e.is_dir)
		);
		assert!(
			root.iter()
				.any(|e| e.rel_path == PathBuf::from("sub") && e.is_dir)
		);
		assert!(
			sub.iter()
				.any(|e| e.rel_path == PathBuf::from("sub/b.txt") && !e.is_dir)
		);
	}

	#[rstest]
	fn reads_file(harness: Harness) {
		// Act
		let bytes = harness.source.read_file(Path::new("a.txt")).unwrap();

		// Assert
		assert_eq!(&*bytes, b"hello");
	}

	#[rstest]
	fn exists_reports_correctly(harness: Harness) {
		// Act + Assert
		assert!(harness.source.exists(Path::new("a.txt")));
		assert!(harness.source.exists(Path::new("sub")));
		assert!(!harness.source.exists(Path::new("missing.txt")));
	}

	#[rstest]
	fn rejects_parent_traversal(harness: Harness) {
		// Act
		let res = harness.source.read_file(Path::new("../escape"));

		// Assert
		assert!(res.is_err());
	}
}
