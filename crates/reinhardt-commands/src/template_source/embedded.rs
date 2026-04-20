//! Embedded template source backed by `rust-embed`.

use super::{TemplateEntry, TemplateSource};
use crate::embedded_templates::TemplateAssets;
use crate::{CommandError, CommandResult};
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

/// Template source backed by the `TemplateAssets` compiled-in archive.
///
/// Scoped to a specific subdirectory of the embedded archive (e.g.
/// `project_restful_template`) so that `list_entries("")` returns
/// only that subdirectory's contents.
#[derive(Debug, Clone)]
pub struct EmbeddedSource {
	subdir: String,
}

impl EmbeddedSource {
	/// Create an `EmbeddedSource` rooted at `subdir` inside the embedded archive.
	pub fn new(subdir: impl Into<String>) -> Self {
		Self {
			subdir: subdir.into(),
		}
	}
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

fn rel_to_embed_key(rel: &Path) -> CommandResult<String> {
	reject_traversal(rel)?;
	let mut key = String::new();
	let mut first = true;
	for comp in rel.components() {
		if let Component::Normal(s) = comp {
			if !first {
				key.push('/');
			}
			key.push_str(&s.to_string_lossy());
			first = false;
		}
	}
	Ok(key)
}

impl TemplateSource for EmbeddedSource {
	fn list_entries(&self, rel: &Path) -> CommandResult<Vec<TemplateEntry>> {
		// Build the full prefix: subdir/rel
		let rel_key = rel_to_embed_key(rel)?;
		let prefix = if rel_key.is_empty() {
			self.subdir.clone()
		} else {
			format!("{}/{}", self.subdir, rel_key)
		};
		let prefix_with_slash = format!("{}/", prefix);

		let mut files: Vec<PathBuf> = Vec::new();
		let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();

		for key in TemplateAssets::iter() {
			let k: &str = key.as_ref();
			let suffix = if let Some(s) = k.strip_prefix(&prefix_with_slash) {
				s
			} else {
				continue;
			};
			if suffix.is_empty() {
				continue;
			}
			match suffix.find('/') {
				None => files.push(rel.join(suffix)),
				Some(idx) => {
					let dir_name = &suffix[..idx];
					dirs.insert(rel.join(dir_name));
				}
			}
		}

		let mut entries: Vec<TemplateEntry> = dirs
			.into_iter()
			.map(|p| TemplateEntry {
				rel_path: p,
				is_dir: true,
			})
			.collect();
		entries.extend(files.into_iter().map(|p| TemplateEntry {
			rel_path: p,
			is_dir: false,
		}));
		Ok(entries)
	}

	fn read_file(&self, rel: &Path) -> CommandResult<Cow<'_, [u8]>> {
		let rel_key = rel_to_embed_key(rel)?;
		let key = format!("{}/{}", self.subdir, rel_key);
		let file = TemplateAssets::get(&key).ok_or_else(|| {
			CommandError::ExecutionError(format!("embedded template not found: {}", key))
		})?;
		Ok(Cow::Owned(file.data.into_owned()))
	}

	fn exists(&self, rel: &Path) -> bool {
		let Ok(rel_key) = rel_to_embed_key(rel) else {
			return false;
		};
		let key = if rel_key.is_empty() {
			self.subdir.clone()
		} else {
			format!("{}/{}", self.subdir, rel_key)
		};
		if TemplateAssets::get(&key).is_some() {
			return true;
		}
		let prefix_with_slash = format!("{}/", key);
		TemplateAssets::iter().any(|k| {
			let k: &str = k.as_ref();
			k.starts_with(&prefix_with_slash)
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[fixture]
	fn source() -> EmbeddedSource {
		EmbeddedSource::new("project_restful_template")
	}

	#[rstest]
	fn lists_entries_in_subdir(source: EmbeddedSource) {
		// Act: list_entries("") returns files/dirs within the scoped subdir
		let entries = source.list_entries(Path::new("")).expect("root listing");

		// Assert: at least one file and no empty paths
		assert!(
			!entries.is_empty(),
			"project_restful_template must have entries"
		);
		assert!(entries.iter().all(|e| !e.rel_path.as_os_str().is_empty()));
		// Paths are relative within the subdir — NOT prefixed with subdir name
		assert!(
			entries
				.iter()
				.all(|e| !e.rel_path.to_str().unwrap_or("").starts_with("project_")),
			"paths must be relative within the subdir"
		);
	}

	#[rstest]
	fn reads_known_file(source: EmbeddedSource) {
		// Arrange: find any file in the scoped listing
		let candidates = source.list_entries(Path::new("")).expect("listing");
		let first_file = candidates
			.into_iter()
			.find(|e| !e.is_dir)
			.expect("at least one file");

		// Act
		let bytes = source.read_file(&first_file.rel_path).expect("read");

		// Assert
		assert!(!bytes.is_empty());
	}

	#[rstest]
	fn exists_true_for_root_subdir(source: EmbeddedSource) {
		// Act + Assert: "" (scoped root) should exist; unknown path should not
		assert!(source.exists(Path::new("")));
		assert!(!source.exists(Path::new("definitely_does_not_exist_xyz")));
	}

	#[rstest]
	fn rejects_parent_traversal(source: EmbeddedSource) {
		// Act
		let res = source.read_file(Path::new("../escape.txt"));

		// Assert
		assert!(res.is_err(), "parent traversal must be rejected");
	}
}
