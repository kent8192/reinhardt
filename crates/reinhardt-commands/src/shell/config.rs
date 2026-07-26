use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::{CommandError, CommandResult};

/// Project configuration required to bootstrap the Rust management shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellConfig {
	package_name: String,
	crate_name: String,
	manifest_dir: PathBuf,
	settings_factory_path: String,
	installed_app_labels: Vec<String>,
	project_prelude: String,
}

impl ShellConfig {
	/// Creates an immutable shell configuration.
	pub fn new<I, L>(
		package_name: impl Into<String>,
		crate_name: impl Into<String>,
		manifest_dir: impl Into<PathBuf>,
		settings_factory_path: impl Into<String>,
		installed_app_labels: I,
	) -> Self
	where
		I: IntoIterator<Item = L>,
		L: Into<String>,
	{
		Self {
			package_name: package_name.into(),
			crate_name: crate_name.into(),
			manifest_dir: manifest_dir.into(),
			settings_factory_path: settings_factory_path.into(),
			installed_app_labels: installed_app_labels.into_iter().map(Into::into).collect(),
			project_prelude: String::new(),
		}
	}

	/// Adds the project-defined source evaluated after the standard shell prelude.
	pub fn with_prelude(mut self, source: impl Into<String>) -> Self {
		self.project_prelude = source.into();
		self
	}

	/// Returns the Cargo package name.
	pub fn package_name(&self) -> &str {
		&self.package_name
	}

	/// Returns the Rust crate name used by evaluated source.
	pub fn crate_name(&self) -> &str {
		&self.crate_name
	}

	/// Returns the unvalidated project manifest directory.
	pub fn manifest_dir(&self) -> &Path {
		&self.manifest_dir
	}

	/// Returns the fully qualified settings factory path.
	pub fn settings_factory_path(&self) -> &str {
		&self.settings_factory_path
	}

	/// Returns installed application labels in declaration order.
	pub fn installed_app_labels(&self) -> &[String] {
		&self.installed_app_labels
	}

	/// Returns the optional project-defined prelude source.
	pub fn project_prelude(&self) -> &str {
		&self.project_prelude
	}

	/// Validates and normalizes the shell configuration.
	pub fn validate(&self) -> CommandResult<ValidatedShellConfig> {
		if !is_rust_identifier(&self.crate_name) {
			return Err(CommandError::InvalidArguments(format!(
				"invalid Rust crate identifier: {:?}",
				self.crate_name
			)));
		}

		let settings_segments: Vec<_> = self.settings_factory_path.split("::").collect();
		if settings_segments.len() < 2 {
			return Err(CommandError::InvalidArguments(
				"settings factory path must be an absolute Rust path".to_string(),
			));
		}
		if settings_segments.iter().any(|segment| {
			segment.is_empty()
				|| !is_rust_identifier(segment)
				|| matches!(*segment, "crate" | "self" | "super" | "Self")
		}) {
			return Err(CommandError::InvalidArguments(
				"settings factory path must contain only Rust identifiers".to_string(),
			));
		}
		if settings_segments[0] != self.crate_name {
			return Err(CommandError::InvalidArguments(format!(
				"settings factory path must start with configured crate name `{}`",
				self.crate_name
			)));
		}

		let manifest_dir = self.manifest_dir.canonicalize()?;
		if !manifest_dir.is_dir() {
			return Err(CommandError::InvalidArguments(format!(
				"manifest directory is not a directory: {}",
				manifest_dir.display()
			)));
		}
		let cargo_manifest = manifest_dir.join("Cargo.toml");
		if !cargo_manifest.is_file() {
			return Err(CommandError::InvalidArguments(format!(
				"Cargo manifest must be a regular file: {}",
				cargo_manifest.display()
			)));
		}
		let mut seen_labels = HashSet::new();
		let installed_app_labels = self
			.installed_app_labels
			.iter()
			.filter(|label| seen_labels.insert(label.as_str()))
			.cloned()
			.collect();

		Ok(ValidatedShellConfig {
			package_name: self.package_name.clone(),
			crate_name: self.crate_name.clone(),
			manifest_dir,
			settings_factory_path: self.settings_factory_path.clone(),
			installed_app_labels,
			project_prelude: self.project_prelude.clone(),
		})
	}
}

fn is_rust_identifier(value: &str) -> bool {
	syn::parse_str::<syn::Ident>(value).is_ok()
}

/// A validated shell configuration with a canonical project directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedShellConfig {
	package_name: String,
	crate_name: String,
	manifest_dir: PathBuf,
	settings_factory_path: String,
	installed_app_labels: Vec<String>,
	project_prelude: String,
}

impl ValidatedShellConfig {
	/// Returns the Cargo package name.
	pub fn package_name(&self) -> &str {
		&self.package_name
	}

	/// Returns the Rust crate name used by evaluated source.
	pub fn crate_name(&self) -> &str {
		&self.crate_name
	}

	/// Returns the canonical project manifest directory.
	pub fn manifest_dir(&self) -> &Path {
		&self.manifest_dir
	}

	/// Returns the fully qualified settings factory path.
	pub fn settings_factory_path(&self) -> &str {
		&self.settings_factory_path
	}

	/// Returns deduplicated installed application labels.
	pub fn installed_app_labels(&self) -> &[String] {
		&self.installed_app_labels
	}

	/// Returns the project-defined source for the final prelude layer.
	pub fn project_prelude(&self) -> &str {
		&self.project_prelude
	}
}
