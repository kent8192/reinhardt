//! Merged template source: primary wins, fallback fills gaps.

use super::{EmbeddedSource, FilesystemSource, TemplateEntry, TemplateSource};
use crate::CommandResult;
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn rendered_path(rel: &Path) -> PathBuf {
	let mut rendered = rel.to_path_buf();
	let Some(file_name) = rendered
		.file_name()
		.and_then(|name| name.to_str())
		.map(str::to_owned)
	else {
		return rendered;
	};
	if let Some(file_name) = file_name.strip_suffix(".tpl") {
		rendered.set_file_name(file_name);
	}
	rendered
}

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

		let mut seen: HashSet<PathBuf> = primary_entries
			.iter()
			.map(|e| rendered_path(&e.rel_path))
			.collect();
		let mut out = primary_entries;
		for e in fallback_entries {
			if seen.insert(rendered_path(&e.rel_path)) {
				out.push(e);
			}
		}
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
		let primary = FilesystemSource::new(tmp.path()).unwrap();
		let fallback = EmbeddedSource::new("project_restful_template");
		Harness {
			_tmp: tmp,
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
				entries
					.iter()
					.any(|m| rendered_path(&m.rel_path) == rendered_path(&e.rel_path)),
				"missing from merged: {:?}",
				e.rel_path
			);
		}
	}

	#[rstest]
	fn exists_checks_both(harness: Harness) {
		// Act + Assert
		assert!(harness.source.exists(Path::new("README.md"))); // primary-only file
		assert!(!harness.source.exists(Path::new("definitely_missing_xyz")));
	}

	#[rstest]
	#[case("project_restful_template")]
	#[case("project_pages_template")]
	fn guidance_files_fall_back_at_project_root(#[case] template_root: &str) {
		// Arrange
		let tmp = TempDir::new().unwrap();
		fs::write(tmp.path().join("README.md"), b"OVERRIDDEN").unwrap();
		let source = MergedSource {
			primary: FilesystemSource::new(tmp.path()).unwrap(),
			fallback: EmbeddedSource::new(template_root),
		};

		// Act
		let entries = source.list_entries(Path::new("")).unwrap();

		// Assert
		for relative_path in ["AGENTS.md.tpl", "CLAUDE.md.tpl"] {
			let path = Path::new(relative_path);
			assert!(
				entries
					.iter()
					.any(|entry| !entry.is_dir && entry.rel_path.as_path() == path),
				"{relative_path} must remain at the project template root"
			);

			let merged = source.read_file(path).unwrap();
			let embedded = source.fallback.read_file(path).unwrap();
			assert_eq!(merged.as_ref(), embedded.as_ref());
		}
	}

	#[test]
	fn literal_primary_guidance_file_wins_over_template_fallback() {
		let tmp = TempDir::new().unwrap();
		fs::write(tmp.path().join("AGENTS.md"), b"CUSTOM GUIDANCE").unwrap();
		let source = MergedSource {
			primary: FilesystemSource::new(tmp.path()).unwrap(),
			fallback: EmbeddedSource::new("project_restful_template"),
		};

		let entries = source.list_entries(Path::new("")).unwrap();

		assert!(
			entries.iter().any(|entry| {
				!entry.is_dir && entry.rel_path.as_path() == Path::new("AGENTS.md")
			})
		);
		assert!(!entries.iter().any(|entry| {
			!entry.is_dir && entry.rel_path.as_path() == Path::new("AGENTS.md.tpl")
		}));
		assert_eq!(
			source.read_file(Path::new("AGENTS.md")).unwrap().as_ref(),
			b"CUSTOM GUIDANCE"
		);
	}
}
