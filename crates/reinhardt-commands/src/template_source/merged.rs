//! Merged template source: primary wins, fallback fills gaps.

use super::{EmbeddedSource, FilesystemSource, TemplateEntry, TemplateSource};
use crate::CommandResult;
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
/// Template source that merges an external filesystem directory with the embedded defaults.
///
/// Files present in `primary` take precedence; everything else falls back to `fallback`.
pub struct MergedSource {
	/// External override directory searched first.
	pub primary: FilesystemSource,
	/// Compiled-in embedded archive used when `primary` does not have the file.
	pub fallback: EmbeddedSource,
}

impl TemplateSource for MergedSource {
	fn list_entries(&self, rel: &Path) -> CommandResult<Vec<TemplateEntry>> {
		let primary_entries: Vec<TemplateEntry> = if self.primary.exists(rel) {
			self.primary.list_entries(rel)?
		} else {
			Vec::new()
		};
		let fallback_entries = self.fallback.list_entries(rel)?;

		let mut seen: HashSet<PathBuf> =
			primary_entries.iter().map(|e| e.rel_path.clone()).collect();
		let mut out = primary_entries;
		for e in fallback_entries {
			if seen.insert(e.rel_path.clone()) {
				out.push(e);
			}
		}
		out.sort_by(|left, right| left.rel_path.cmp(&right.rel_path));
		out.dedup_by(|left, right| left.rel_path == right.rel_path);
		Ok(out)
	}

	fn read_file(&self, rel: &Path) -> CommandResult<Cow<'_, [u8]>> {
		if self.primary.exists(rel) {
			return self.primary.read_file(rel);
		}
		self.fallback.read_file(rel)
	}

	fn exists(&self, rel: &Path) -> bool {
		self.primary.exists(rel) || self.fallback.exists(rel)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::template_source::EmbeddedSource;
	use rstest::*;
	use std::fs;
	use std::path::Path;
	use tempfile::TempDir;

	struct Harness {
		_tmp: TempDir,
		primary_root: std::path::PathBuf,
		source: MergedSource,
	}

	/// Both primary and fallback are scoped to "project_restful_template".
	/// The primary has a single override file (README.md); everything else falls back to embedded.
	#[fixture]
	fn harness() -> Harness {
		let tmp = TempDir::new().unwrap();
		// primary is rooted at tmp/project_restful_template/ directly
		fs::create_dir_all(tmp.path()).unwrap();
		fs::write(tmp.path().join("README.md"), b"OVERRIDDEN").unwrap();
		let primary_root = tmp.path().to_path_buf();
		let primary = FilesystemSource::new(tmp.path()).unwrap();
		let fallback = EmbeddedSource::new("project_restful_template");
		Harness {
			_tmp: tmp,
			primary_root,
			source: MergedSource { primary, fallback },
		}
	}

	#[rstest]
	fn primary_wins_when_present(harness: Harness) {
		// Act
		let bytes = harness.source.read_file(Path::new("README.md")).unwrap();

		// Assert
		assert_eq!(&*bytes, b"OVERRIDDEN");
	}

	#[rstest]
	fn falls_back_to_embedded_when_primary_missing(harness: Harness) {
		// Arrange: find a file in embedded that is NOT in the primary (override dir).
		let embedded = EmbeddedSource::new("project_restful_template");
		let candidates = embedded.list_entries(Path::new("")).unwrap();
		let missing_in_primary = candidates
			.iter()
			.find(|e| !e.is_dir && e.rel_path.as_path() != Path::new("README.md"))
			.expect("embedded has more than README.md");

		// Act
		let via_merged = harness
			.source
			.read_file(&missing_in_primary.rel_path)
			.unwrap();
		let via_embedded = embedded.read_file(&missing_in_primary.rel_path).unwrap();

		// Assert
		assert_eq!(&*via_merged, &*via_embedded);
	}

	#[rstest]
	fn list_unions_with_primary_priority(harness: Harness) {
		// Act
		let entries = harness.source.list_entries(Path::new("")).unwrap();

		// Assert: primary's README.md must be present
		assert!(
			entries
				.iter()
				.any(|e| e.rel_path.as_path() == Path::new("README.md"))
		);
		// Every embedded entry must also appear
		let embedded = EmbeddedSource::new("project_restful_template");
		for e in embedded.list_entries(Path::new("")).unwrap() {
			assert!(
				entries.iter().any(|m| m.rel_path == e.rel_path),
				"missing from merged: {:?}",
				e.rel_path
			);
		}
	}

	#[rstest]
	fn list_entries_is_sorted_and_deduplicated(harness: Harness) {
		// Arrange
		fs::write(harness.primary_root.join("zzz-override.txt"), b"last").unwrap();
		fs::write(harness.primary_root.join("aaa-override.txt"), b"first").unwrap();

		// Act
		let entries = harness.source.list_entries(Path::new("")).unwrap();
		let paths = entries
			.iter()
			.map(|entry| entry.rel_path.clone())
			.collect::<Vec<_>>();

		// Assert
		let mut sorted = paths.clone();
		sorted.sort();
		sorted.dedup();
		assert_eq!(paths, sorted);
	}

	#[rstest]
	fn exists_checks_both(harness: Harness) {
		// Act + Assert
		assert!(harness.source.exists(Path::new("README.md"))); // primary-only file
		assert!(!harness.source.exists(Path::new("definitely_missing_xyz")));
	}

	#[rstest]
	fn missing_file_returns_the_fallback_source_error(harness: Harness) {
		// Act
		let error = harness
			.source
			.read_file(Path::new("definitely-missing.txt"))
			.unwrap_err();

		// Assert
		assert_eq!(
			error.to_string(),
			"Execution error: embedded template not found: project_restful_template/definitely-missing.txt"
		);
	}
}
