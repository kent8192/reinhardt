//! Upgrade generated migration source files to the current source format.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Args;
use reinhardt_db::migrations::upgrade_source;
use syn::{File, Item};
use thiserror::Error;
use walkdir::WalkDir;

/// Arguments for `reinhardt-admin migrations upgrade-source`.
#[derive(Args, Debug)]
pub struct UpgradeSourceArgs {
	/// Migration directory or source file to inspect.
	#[arg(default_value = "migrations", value_name = "PATH")]
	pub path: PathBuf,

	/// Check for upgrades without writing files; exits unsuccessfully when drift exists.
	#[arg(long)]
	pub check: bool,
}

/// Errors produced while upgrading migration source files.
#[derive(Debug, Error)]
pub enum MigrationSourceError {
	/// The requested path does not exist or cannot be read.
	#[error("I/O operation failed: {0}")]
	Io(#[from] std::io::Error),
	/// A source tree entry could not be enumerated.
	#[error("failed to walk migration sources: {0}")]
	Walk(#[from] walkdir::Error),
	/// One or more files failed the preflight pass.
	#[error("migration source upgrade preflight failed:\n{0}")]
	Preflight(String),
	/// A write failed after preflight completed.
	#[error("failed to write upgraded migration source: {0}")]
	Write(String),
}

/// Result type for migration source upgrades.
pub type Result<T> = std::result::Result<T, MigrationSourceError>;

struct PlannedUpgrade {
	path: PathBuf,
	relative_path: PathBuf,
	source: String,
}

/// Upgrade all generated migration sources below `args.path`.
///
/// The complete tree is parsed and converted before any destination is
/// touched. `--check` performs the same preflight and reports drift without
/// writing files.
pub fn run(args: UpgradeSourceArgs) -> Result<()> {
	let root = validate_root(&args.path)?;
	let files = find_source_files(&root, args.path.is_file())?;
	let mut plans = Vec::new();
	let mut diagnostics = Vec::new();

	for path in files {
		let relative_path = path
			.strip_prefix(&root)
			.ok()
			.filter(|relative| !relative.as_os_str().is_empty())
			.map(Path::to_path_buf)
			.unwrap_or_else(|| path.file_name().map_or_else(PathBuf::new, PathBuf::from));
		let source = match fs::read_to_string(&path) {
			Ok(source) => source,
			Err(error) => {
				diagnostics.push(format!("{}: {error}", relative_path.display()));
				continue;
			}
		};
		let file = match syn::parse_file(&source) {
			Ok(file) => file,
			Err(error) => {
				diagnostics.push(format!(
					"{}: failed to parse Rust source: {error}",
					relative_path.display()
				));
				continue;
			}
		};
		if !defines_migration(&file) {
			continue;
		}
		match upgrade_source(&source) {
			Ok(result) if result.changed => plans.push(PlannedUpgrade {
				path,
				relative_path,
				source: result.source,
			}),
			Ok(_) => {}
			Err(error) => diagnostics.push(format!("{}: {error}", relative_path.display())),
		}
	}

	if !diagnostics.is_empty() {
		diagnostics.sort();
		return Err(MigrationSourceError::Preflight(diagnostics.join("\n")));
	}

	plans.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
	if args.check {
		for plan in &plans {
			println!("would upgrade: {}", plan.relative_path.display());
		}
		if plans.is_empty() {
			println!("migration source format is current");
			return Ok(());
		}
		return Err(MigrationSourceError::Preflight(format!(
			"{} migration source file(s) require upgrade",
			plans.len()
		)));
	}

	for plan in &plans {
		write_atomically(&plan.path, &plan.source)?;
		println!("upgraded: {}", plan.relative_path.display());
	}
	println!("{} migration source file(s) upgraded", plans.len());
	Ok(())
}

fn validate_root(path: &Path) -> Result<PathBuf> {
	let metadata = fs::symlink_metadata(path)?;
	if metadata.file_type().is_symlink() {
		return Err(MigrationSourceError::Preflight(format!(
			"{}: symlinks are not accepted",
			path.display()
		)));
	}
	let root = path.canonicalize()?;
	if !root.is_dir() && !root.is_file() {
		return Err(MigrationSourceError::Preflight(format!(
			"{}: expected a directory or Rust source file",
			path.display()
		)));
	}
	if root.is_dir() && root.join("Cargo.toml").is_file() {
		return Err(MigrationSourceError::Preflight(format!(
			"{}: refusing to scan a repository root; pass a migrations directory or source file",
			root.display()
		)));
	}
	if root.is_file() && root.extension().is_none_or(|extension| extension != "rs") {
		return Err(MigrationSourceError::Preflight(format!(
			"{}: expected a .rs source file",
			root.display()
		)));
	}
	Ok(root)
}

fn find_source_files(root: &Path, single_file: bool) -> Result<Vec<PathBuf>> {
	if single_file {
		return Ok(vec![root.to_path_buf()]);
	}

	let mut files = Vec::new();
	for entry in WalkDir::new(root).follow_links(false) {
		let entry = entry?;
		let path = entry.path();
		if path != root && entry.file_type().is_symlink() {
			return Err(MigrationSourceError::Preflight(format!(
				"{}: symlinks are not accepted",
				path.display()
			)));
		}
		if !entry.file_type().is_file()
			|| path.extension().is_none_or(|extension| extension != "rs")
		{
			continue;
		}
		let canonical = path.canonicalize()?;
		if !canonical.starts_with(root) {
			return Err(MigrationSourceError::Preflight(format!(
				"{}: path escapes the requested root",
				path.display()
			)));
		}
		files.push(canonical);
	}
	files.sort();
	Ok(files)
}

fn defines_migration(file: &File) -> bool {
	file.items
		.iter()
		.any(|item| matches!(item, Item::Fn(function) if function.sig.ident == "migration"))
}

fn write_atomically(path: &Path, content: &str) -> Result<()> {
	let metadata = fs::symlink_metadata(path)?;
	if metadata.file_type().is_symlink() {
		return Err(MigrationSourceError::Write(format!(
			"{}: destination became a symlink",
			path.display()
		)));
	}
	let parent = path.parent().ok_or_else(|| {
		MigrationSourceError::Write(format!("{}: destination has no parent", path.display()))
	})?;
	let name = path
		.file_name()
		.and_then(|name| name.to_str())
		.ok_or_else(|| {
			MigrationSourceError::Write(format!(
				"{}: destination has no valid file name",
				path.display()
			))
		})?;
	let nonce = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap_or_default()
		.as_nanos();
	let temporary = parent.join(format!(".{name}.{}.{}.tmp", std::process::id(), nonce));
	let write_result = (|| -> std::io::Result<()> {
		let mut file = OpenOptions::new()
			.write(true)
			.create_new(true)
			.open(&temporary)?;
		file.write_all(content.as_bytes())?;
		file.sync_all()?;
		fs::rename(&temporary, path)?;
		Ok(())
	})();
	if let Err(error) = write_result {
		let _ = fs::remove_file(&temporary);
		return Err(MigrationSourceError::Write(format!(
			"{}: {error}",
			path.display()
		)));
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn check_reports_drift_without_writing() {
		let directory = tempfile::tempdir().expect("temporary directory");
		let path = directory.path().join("0001_initial.rs");
		let original = "fn migration() -> Migration { Migration { name: \"0001\".into(), app_label: \"app\".into(), operations: vec![], dependencies: vec![], replaces: vec![], atomic: true, initial: None, state_only: false, database_only: false, swappable_dependencies: vec![], optional_dependencies: vec![] } }\n";
		fs::write(&path, original).expect("write fixture");

		let error = run(UpgradeSourceArgs {
			path: directory.path().to_path_buf(),
			check: true,
		})
		.expect_err("check must report legacy drift");

		assert!(error.to_string().contains("require upgrade"));
		assert_eq!(fs::read_to_string(path).expect("read fixture"), original);
	}
}
