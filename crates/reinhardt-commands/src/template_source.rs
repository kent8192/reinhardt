//! Abstraction over template storage so that commands can read templates from
//! either embedded (`rust-embed`) assets, a user-provided filesystem directory,
//! or a merged view that prefers the filesystem and falls back to embedded.

use crate::CommandResult;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

pub mod embedded;
pub mod filesystem;
pub mod merged;

pub use embedded::EmbeddedSource;
pub use filesystem::FilesystemSource;
pub use merged::MergedSource;

/// A single entry inside a template tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateEntry {
	/// Path relative to the source root.
	pub rel_path: PathBuf,
	/// Whether this entry is a directory. Files have `is_dir == false`.
	pub is_dir: bool,
}

/// A readable template tree rooted at an implementation-defined location.
///
/// Paths passed to these methods are always relative to the source root.
/// Implementations MUST reject any path that escapes the root (e.g. contains
/// `..` components after normalization).
pub trait TemplateSource: Send + Sync {
	/// List immediate children (files and subdirectories) of `rel`.
	///
	/// `rel` is relative to the source root. Use `Path::new("")` for the root.
	fn list_entries(&self, rel: &Path) -> CommandResult<Vec<TemplateEntry>>;

	/// Read the full contents of the file at `rel`.
	///
	/// Returns an error if `rel` does not exist or refers to a directory.
	fn read_file(&self, rel: &Path) -> CommandResult<Cow<'_, [u8]>>;

	/// Report whether a file or directory exists at `rel`.
	fn exists(&self, rel: &Path) -> bool;
}
